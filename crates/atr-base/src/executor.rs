//! Base transaction executor

use alloy::consensus::{SignableTransaction, TxEip1559};
use alloy::eips::eip2718::Encodable2718;
use alloy::primitives::{Address, Bytes, TxKind, U256};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::SignerSync;
use async_trait::async_trait;
use std::str::FromStr;
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
    gas_estimator: GasEstimator,
    nonce_manager: NonceManager,
    signer: Option<PrivateKeySigner>,
}

impl BaseExecutor {
    /// Create a new Base executor
    pub fn new(rpc_url: String) -> Self {
        let gas_estimator = GasEstimator::new(rpc_url);
        let nonce_manager = NonceManager::new();
        Self {
            gas_estimator,
            nonce_manager,
            signer: None,
        }
    }

    /// Set the sender address (derived from signer if signer is set)
    pub fn with_sender(self, _address: String) -> Self {
        // Sender is now derived from signer
        self
    }

    /// Set the signing key from a hex-encoded private key
    pub fn with_private_key(mut self, hex_key: &str) -> AtrResult<Self> {
        let signer: PrivateKeySigner = hex_key
            .parse()
            .map_err(|e| AtrError::ConfigError(format!("Invalid private key: {}", e)))?;
        self.signer = Some(signer);
        Ok(self)
    }

    /// Get the sender address
    fn sender_address(&self) -> AtrResult<Address> {
        self.signer
            .as_ref()
            .map(|s| s.address())
            .ok_or_else(|| AtrError::ConfigError("Signer not configured".to_string()))
    }

    /// Get sender address as hex string
    fn sender_hex(&self) -> AtrResult<String> {
        Ok(format!("{}", self.sender_address()?))
    }

    /// Extract destination address from intent
    fn extract_to_address(operation: &IntentOperation) -> AtrResult<Option<Address>> {
        match operation {
            IntentOperation::Transfer { to, .. }
            | IntentOperation::ContractCall { contract: to, .. }
            | IntentOperation::Swap { dex: to, .. } => {
                let addr = Address::from_str(to).map_err(|e| {
                    AtrError::InvalidIntent(format!("Invalid address '{}': {}", to, e))
                })?;
                Ok(Some(addr))
            }
            IntentOperation::Deploy { .. } => Ok(None), // Contract creation
            IntentOperation::Raw { .. } => Err(AtrError::InvalidIntent(
                "Raw operations require destination in data".to_string(),
            )),
        }
    }

    /// Extract value from intent
    fn extract_value(operation: &IntentOperation) -> U256 {
        match operation {
            IntentOperation::Transfer { amount, .. } => U256::from(*amount),
            IntentOperation::ContractCall { value, .. } => {
                value.map(U256::from).unwrap_or(U256::ZERO)
            }
            _ => U256::ZERO,
        }
    }

    /// Extract calldata from intent
    fn extract_calldata(operation: &IntentOperation) -> Bytes {
        match operation {
            IntentOperation::ContractCall { method, .. } => {
                // In production, use ABI encoding. For now, treat method as hex selector.
                let hex_str = method.trim_start_matches("0x");
                Bytes::from(hex::decode(hex_str).unwrap_or_default())
            }
            IntentOperation::Deploy {
                bytecode,
                constructor_args: _,
            } => {
                let hex_str = bytecode.trim_start_matches("0x");
                Bytes::from(hex::decode(hex_str).unwrap_or_default())
            }
            IntentOperation::Raw { data } => Bytes::from(data.clone()),
            _ => Bytes::new(),
        }
    }
}

#[async_trait]
impl Executor for BaseExecutor {
    async fn simulate(&self, intent: &TransactionIntent) -> AtrResult<SimulationResult> {
        debug!("Simulating Base transaction for intent {}", intent.id);

        let sender = self.sender_hex()?;
        let to_addr = Self::extract_to_address(&intent.operation)?;
        let to_str = to_addr.map(|a| format!("{}", a)).unwrap_or_default();
        let value = match &intent.operation {
            IntentOperation::Transfer { amount, .. } => Some(*amount),
            IntentOperation::ContractCall { value, .. } => *value,
            _ => None,
        };
        let calldata = Self::extract_calldata(&intent.operation);
        let calldata_hex = if calldata.is_empty() {
            None
        } else {
            Some(format!("0x{}", hex::encode(&calldata)))
        };

        let sim_result = self
            .gas_estimator
            .eth_call(&sender, &to_str, value, calldata_hex.as_deref())
            .await;

        match sim_result {
            Ok(_) => {
                let estimated_gas = self
                    .gas_estimator
                    .estimate_gas_for_tx(&sender, &to_str, value, calldata_hex.as_deref())
                    .await
                    .unwrap_or(100_000);

                let base_fee = self
                    .gas_estimator
                    .estimate_base_fee()
                    .await
                    .unwrap_or(1_000_000_000);
                let priority_fee = self
                    .gas_estimator
                    .estimate_priority_fee()
                    .await
                    .unwrap_or(1_000_000_000);
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

        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| AtrError::ConfigError("Signer not configured".to_string()))?;

        let sender_hex = self.sender_hex()?;
        let to_addr = Self::extract_to_address(&intent.operation)?;
        let value = Self::extract_value(&intent.operation);
        let calldata = Self::extract_calldata(&intent.operation);

        // Get nonce
        let on_chain_nonce = self
            .gas_estimator
            .get_transaction_count(&sender_hex)
            .await?;
        self.nonce_manager.sync_nonce(&sender_hex, on_chain_nonce).await;
        let nonce = self.nonce_manager.get_next_nonce(&sender_hex);

        // Estimate fees
        let to_str = to_addr.map(|a| format!("{}", a)).unwrap_or_default();
        let value_u64 = match &intent.operation {
            IntentOperation::Transfer { amount, .. } => Some(*amount),
            IntentOperation::ContractCall {
                value: v, ..
            } => *v,
            _ => None,
        };
        let calldata_hex = if calldata.is_empty() {
            None
        } else {
            Some(format!("0x{}", hex::encode(&calldata)))
        };

        let gas_limit = self
            .gas_estimator
            .estimate_gas_for_tx(&sender_hex, &to_str, value_u64, calldata_hex.as_deref())
            .await
            .unwrap_or(100_000);
        let base_fee = self
            .gas_estimator
            .estimate_base_fee()
            .await
            .unwrap_or(1_000_000_000);
        let priority_fee = self
            .gas_estimator
            .estimate_priority_fee()
            .await
            .unwrap_or(1_000_000_000);

        // Check fee cap
        let max_fee_per_gas = base_fee.saturating_mul(2) + priority_fee; // 2x base fee headroom
        let estimated_total = gas_limit * max_fee_per_gas;
        if let Some(max_fee) = intent.max_fee {
            if estimated_total > max_fee {
                self.nonce_manager.reset_nonce(&sender_hex, nonce);
                return Err(AtrError::InvalidIntent(format!(
                    "Estimated fee {} exceeds max fee {}",
                    estimated_total, max_fee
                )));
            }
        }

        // Build EIP-1559 transaction
        let to = match to_addr {
            Some(addr) => TxKind::Call(addr),
            None => TxKind::Create,
        };

        let mut tx = TxEip1559 {
            chain_id: 8453, // Base mainnet
            nonce,
            gas_limit,
            max_fee_per_gas: max_fee_per_gas as u128,
            max_priority_fee_per_gas: priority_fee as u128,
            to,
            value,
            input: calldata,
            access_list: Default::default(),
        };

        // Sign the transaction
        let sig = signer
            .sign_hash_sync(&tx.signature_hash())
            .map_err(|e| AtrError::Internal(format!("Signing failed: {}", e)))?;
        let signed = tx.into_signed(sig);

        // RLP encode as EIP-2718 envelope
        let mut encoded = Vec::new();
        signed.encode_2718(&mut encoded);
        let raw_hex = format!("0x{}", hex::encode(&encoded));

        // Submit
        let tx_hash = self.gas_estimator.send_raw_transaction(&raw_hex).await?;
        info!("Transaction submitted: {}", tx_hash);
        Ok(tx_hash)
    }

    async fn check_status(&self, tx_hash: &str) -> AtrResult<TransactionRecord> {
        debug!("Checking status for Base transaction {}", tx_hash);

        let receipt = self.gas_estimator.get_transaction_receipt(tx_hash).await?;
        let mut record = TransactionRecord::new(uuid::Uuid::new_v4(), Chain::Base);
        record.tx_hash = Some(tx_hash.to_string());

        match receipt {
            Some(receipt_data) => {
                let status_hex = receipt_data
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("0x0");
                let success = status_hex == "0x1";

                if success {
                    if let Some(block_hex) =
                        receipt_data.get("blockNumber").and_then(|b| b.as_str())
                    {
                        let block_num =
                            u64::from_str_radix(block_hex.trim_start_matches("0x"), 16)
                                .unwrap_or(0);
                        record.block_number = Some(block_num);

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

                    if let Some(gas_hex) = receipt_data.get("gasUsed").and_then(|g| g.as_str()) {
                        record.units_used =
                            u64::from_str_radix(gas_hex.trim_start_matches("0x"), 16).ok();
                    }
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

        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| AtrError::ConfigError("Signer not configured".to_string()))?;

        // To cancel: send a 0-value self-transfer with same nonce but higher gas
        // We need to know the original nonce — for now, return error
        Err(AtrError::Internal(
            "Cancellation requires the original transaction nonce. Use check_status to monitor instead.".to_string(),
        ))
    }
}
