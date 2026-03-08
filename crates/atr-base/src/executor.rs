//! Base transaction executor

use async_trait::async_trait;
use tracing::{debug, info, warn};

use atr_core::{
    chain::Chain,
    error::{AtrError, AtrResult},
    executor::Executor,
    intent::{IntentOperation, TransactionIntent},
    transaction::{SimulationResult, TransactionRecord, TransactionStatus},
};

use crate::gas_estimator::GasEstimator;
use crate::nonce_manager::NonceManager;

/// Base-specific transaction executor
pub struct BaseExecutor {
    rpc_url: String,
    gas_estimator: GasEstimator,
    nonce_manager: NonceManager,
    /// Hex-encoded sender address (0x...)
    sender_address: Option<String>,
}

impl BaseExecutor {
    /// Create a new Base executor
    pub fn new(rpc_url: String) -> Self {
        let gas_estimator = GasEstimator::new(rpc_url.clone());
        let nonce_manager = NonceManager::new();
        Self {
            rpc_url,
            gas_estimator,
            nonce_manager,
            sender_address: None,
        }
    }

    /// Set the sender address for this executor
    pub fn with_sender(mut self, address: String) -> Self {
        self.sender_address = Some(address);
        self
    }

    /// Get the sender address or return a config error
    fn sender(&self) -> AtrResult<&str> {
        self.sender_address
            .as_deref()
            .ok_or_else(|| AtrError::ConfigError("Sender address not configured".to_string()))
    }

    /// Extract destination address from intent operation
    fn extract_to_address(operation: &IntentOperation) -> AtrResult<String> {
        match operation {
            IntentOperation::Transfer { to, .. } => Ok(to.clone()),
            IntentOperation::ContractCall { contract, .. } => Ok(contract.clone()),
            IntentOperation::Swap { dex, .. } => Ok(dex.clone()),
            IntentOperation::Deploy { .. } => {
                // Contract deployment has no "to" address
                Ok(String::new())
            }
            IntentOperation::Raw { .. } => Err(AtrError::InvalidIntent(
                "Raw operations must specify destination in data".to_string(),
            )),
        }
    }

    /// Extract value from intent operation
    fn extract_value(operation: &IntentOperation) -> Option<u64> {
        match operation {
            IntentOperation::Transfer { amount, .. } => Some(*amount),
            IntentOperation::ContractCall { value, .. } => *value,
            _ => None,
        }
    }

    /// Extract calldata from intent operation
    fn extract_calldata(operation: &IntentOperation) -> Option<String> {
        match operation {
            IntentOperation::ContractCall { method, args, .. } => {
                // Build basic calldata from method signature and args
                // In production, this would use ABI encoding
                Some(format!("0x{}", method))
            }
            IntentOperation::Deploy { bytecode, .. } => Some(bytecode.clone()),
            IntentOperation::Raw { data } => Some(format!("0x{}", hex::encode(data))),
            _ => None,
        }
    }
}

#[async_trait]
impl Executor for BaseExecutor {
    async fn simulate(&self, intent: &TransactionIntent) -> AtrResult<SimulationResult> {
        debug!("Simulating Base transaction for intent {}", intent.id);

        let sender = self.sender()?;
        let to = Self::extract_to_address(&intent.operation)?;
        let value = Self::extract_value(&intent.operation);
        let calldata = Self::extract_calldata(&intent.operation);

        // Use eth_call to simulate
        let sim_result = self
            .gas_estimator
            .eth_call(sender, &to, value, calldata.as_deref())
            .await;

        match sim_result {
            Ok(_trace) => {
                // Estimate gas for this specific transaction
                let estimated_gas = self
                    .gas_estimator
                    .estimate_gas_for_tx(sender, &to, value, calldata.as_deref())
                    .await
                    .unwrap_or(100_000);

                let base_fee = self.gas_estimator.estimate_base_fee().await.unwrap_or(1_000_000_000);
                let priority_fee = self.gas_estimator.estimate_priority_fee().await.unwrap_or(1_000_000_000);
                let l1_cost = self.gas_estimator.estimate_l1_cost().await.unwrap_or(10_000);

                let estimated_fee = estimated_gas * (base_fee + priority_fee) + l1_cost;

                Ok(SimulationResult {
                    success: true,
                    estimated_units: Some(estimated_gas),
                    estimated_fee: Some(estimated_fee),
                    error: None,
                    trace: None,
                })
            }
            Err(e) => {
                warn!("Simulation failed for intent {}: {}", intent.id, e);
                Ok(SimulationResult {
                    success: false,
                    estimated_units: None,
                    estimated_fee: None,
                    error: Some(e.to_string()),
                    trace: None,
                })
            }
        }
    }

    async fn submit(&self, intent: &TransactionIntent) -> AtrResult<String> {
        info!("Submitting Base transaction for intent {}", intent.id);

        let sender = self.sender()?;
        let to = Self::extract_to_address(&intent.operation)?;
        let value = Self::extract_value(&intent.operation);
        let calldata = Self::extract_calldata(&intent.operation);

        // 1. Get nonce
        let on_chain_nonce = self.gas_estimator.get_transaction_count(sender).await?;
        self.nonce_manager.sync_nonce(sender, on_chain_nonce).await;
        let nonce = self.nonce_manager.get_next_nonce(sender);

        // 2. Estimate fees
        let gas_limit = self
            .gas_estimator
            .estimate_gas_for_tx(sender, &to, value, calldata.as_deref())
            .await
            .unwrap_or(100_000);
        let base_fee = self.gas_estimator.estimate_base_fee().await.unwrap_or(1_000_000_000);
        let priority_fee = self
            .gas_estimator
            .estimate_priority_fee()
            .await
            .unwrap_or(1_000_000_000);

        // Check fee cap
        let estimated_fee = gas_limit * (base_fee + priority_fee);
        if let Some(max_fee) = intent.max_fee {
            if estimated_fee > max_fee {
                // Reset nonce since we're not submitting
                self.nonce_manager.reset_nonce(sender, nonce);
                return Err(AtrError::InvalidIntent(format!(
                    "Estimated fee {} exceeds max fee {}",
                    estimated_fee, max_fee
                )));
            }
        }

        // 3. Build EIP-1559 transaction
        // Note: In production, this would use alloy's TransactionRequest + signing
        // For now, we build the raw transaction parameters and log them
        info!(
            "Built EIP-1559 tx: nonce={}, gas_limit={}, max_fee_per_gas={}, max_priority_fee={}, to={}, value={:?}",
            nonce, gas_limit, base_fee + priority_fee, priority_fee, to, value
        );

        // The actual signing and submission requires a private key.
        // In production: use alloy's SignerMiddleware or a KMS signer.
        // For now, if we have raw transaction data, submit it directly.
        match &intent.operation {
            IntentOperation::Raw { data } => {
                let raw_hex = format!("0x{}", hex::encode(data));
                let tx_hash = self.gas_estimator.send_raw_transaction(&raw_hex).await?;
                info!("Transaction submitted: {}", tx_hash);
                Ok(tx_hash)
            }
            _ => {
                // Without a signer, we can't sign and submit arbitrary transactions
                // Return the transaction parameters as a structured error so the caller
                // can sign externally
                Err(AtrError::ConfigError(format!(
                    "Transaction built but signer not configured. Params: nonce={}, gas={}, to={}, chain_id=8453",
                    nonce, gas_limit, to
                )))
            }
        }
    }

    async fn check_status(&self, tx_hash: &str) -> AtrResult<TransactionRecord> {
        debug!("Checking status for Base transaction {}", tx_hash);

        let receipt = self.gas_estimator.get_transaction_receipt(tx_hash).await?;

        let mut record = TransactionRecord::new(uuid::Uuid::new_v4(), Chain::Base);
        record.tx_hash = Some(tx_hash.to_string());

        match receipt {
            Some(receipt_data) => {
                // Parse receipt status
                let status_hex = receipt_data
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("0x0");

                let success = status_hex == "0x1";

                if success {
                    // Get block number
                    if let Some(block_hex) = receipt_data.get("blockNumber").and_then(|b| b.as_str())
                    {
                        let block_num = u64::from_str_radix(
                            block_hex.trim_start_matches("0x"),
                            16,
                        )
                        .unwrap_or(0);
                        record.block_number = Some(block_num);

                        // Check confirmations
                        let current_block =
                            self.gas_estimator.get_block_number().await.unwrap_or(0);
                        let confirmations = current_block.saturating_sub(block_num);

                        if confirmations >= 12 {
                            record.update_status(TransactionStatus::Finalized);
                        } else {
                            record.update_status(TransactionStatus::Confirmed);
                        }
                    } else {
                        record.update_status(TransactionStatus::Confirmed);
                    }

                    // Parse gas used
                    if let Some(gas_hex) =
                        receipt_data.get("gasUsed").and_then(|g| g.as_str())
                    {
                        record.units_used = u64::from_str_radix(
                            gas_hex.trim_start_matches("0x"),
                            16,
                        )
                        .ok();
                    }

                    // Parse effective gas price for fee calculation
                    if let Some(price_hex) = receipt_data
                        .get("effectiveGasPrice")
                        .and_then(|p| p.as_str())
                    {
                        if let (Some(gas_used), Ok(price)) = (
                            record.units_used,
                            u64::from_str_radix(price_hex.trim_start_matches("0x"), 16),
                        ) {
                            record.fee_paid = Some(gas_used.saturating_mul(price));
                        }
                    }
                } else {
                    record.update_status(TransactionStatus::Failed);
                    record.error = Some("Transaction reverted on-chain".to_string());
                }
            }
            None => {
                // No receipt yet — still pending
                record.update_status(TransactionStatus::Submitted);
            }
        }

        Ok(record)
    }

    async fn estimate_fee(&self) -> AtrResult<u64> {
        let gas = self.gas_estimator.estimate_gas().await?;
        let base_fee = self.gas_estimator.estimate_base_fee().await?;
        let priority_fee = self.gas_estimator.estimate_priority_fee().await?;
        let l1_cost = self.gas_estimator.estimate_l1_cost().await?;
        Ok(gas * (base_fee + priority_fee) + l1_cost)
    }

    async fn cancel(&self, tx_hash: &str) -> AtrResult<String> {
        info!("Cancelling Base transaction {}", tx_hash);

        let sender = self.sender()?;

        // To cancel on EVM: submit a 0-value self-transfer with the same nonce but higher gas
        // We need to find the original nonce from the pending transaction
        // For now, this requires the Raw intent path (pre-signed replacement tx)

        Err(AtrError::ConfigError(
            "Cancellation requires a signer. Submit a pre-signed replacement transaction via Raw intent.".to_string(),
        ))
    }
}
