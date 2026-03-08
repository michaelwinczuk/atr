//! Solana transaction executor

use async_trait::async_trait;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::Signature,
    system_instruction,
    transaction::Transaction,
};
use std::str::FromStr;
use tracing::{debug, info, warn};

use atr_core::{
    chain::Chain,
    error::{AtrError, AtrResult},
    executor::Executor,
    intent::{IntentOperation, TransactionIntent},
    transaction::{SimulationResult, TransactionRecord, TransactionStatus},
};

use crate::fee_estimator::PriorityFeeEstimator;

/// Solana-specific transaction executor
pub struct SolanaExecutor {
    rpc_client: RpcClient,
    fee_estimator: PriorityFeeEstimator,
    commitment: CommitmentConfig,
}

impl SolanaExecutor {
    /// Create a new Solana executor
    pub fn new(rpc_url: String, commitment: CommitmentConfig) -> Self {
        let rpc_client = RpcClient::new_with_commitment(rpc_url.clone(), commitment);
        let fee_estimator = PriorityFeeEstimator::new(rpc_url);
        Self {
            rpc_client,
            fee_estimator,
            commitment,
        }
    }

    /// Build a Solana transaction from intent
    fn build_transaction(&self, intent: &TransactionIntent) -> AtrResult<Transaction> {
        let mut instructions = Vec::new();

        // Add priority fee instruction based on network conditions
        // We use a default here; the caller should set compute budget after simulation
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(200_000));
        instructions.push(ComputeBudgetInstruction::set_compute_unit_price(5000));

        match &intent.operation {
            IntentOperation::Transfer { to, amount } => {
                let from_pubkey = Pubkey::default(); // Placeholder — will be set by signer
                let to_pubkey = Pubkey::from_str(to).map_err(|e| {
                    AtrError::InvalidIntent(format!("Invalid Solana address '{}': {}", to, e))
                })?;

                instructions.push(system_instruction::transfer(
                    &from_pubkey,
                    &to_pubkey,
                    *amount,
                ));
            }
            IntentOperation::ContractCall {
                contract,
                method: _,
                args,
                value: _,
            } => {
                let program_id = Pubkey::from_str(contract).map_err(|e| {
                    AtrError::InvalidIntent(format!(
                        "Invalid program address '{}': {}",
                        contract, e
                    ))
                })?;

                // Parse accounts and data from args
                let data = if let Some(data_str) = args.get("data").and_then(|d| d.as_str()) {
                    hex::decode(data_str.trim_start_matches("0x")).map_err(|e| {
                        AtrError::InvalidIntent(format!("Invalid instruction data: {}", e))
                    })?
                } else {
                    Vec::new()
                };

                let accounts: Vec<AccountMeta> = if let Some(accts) =
                    args.get("accounts").and_then(|a| a.as_array())
                {
                    accts
                        .iter()
                        .filter_map(|a| {
                            let pubkey_str = a.get("pubkey")?.as_str()?;
                            let pubkey = Pubkey::from_str(pubkey_str).ok()?;
                            let is_signer = a
                                .get("isSigner")
                                .and_then(|s| s.as_bool())
                                .unwrap_or(false);
                            let is_writable = a
                                .get("isWritable")
                                .and_then(|w| w.as_bool())
                                .unwrap_or(false);
                            if is_writable {
                                Some(AccountMeta::new(pubkey, is_signer))
                            } else {
                                Some(AccountMeta::new_readonly(pubkey, is_signer))
                            }
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                instructions.push(Instruction::new_with_bytes(program_id, &data, accounts));
            }
            IntentOperation::Raw { data } => {
                // Raw instruction data: first 32 bytes = program ID, rest = instruction data
                if data.len() < 32 {
                    return Err(AtrError::InvalidIntent(
                        "Raw data must contain at least 32 bytes for program ID".to_string(),
                    ));
                }
                let program_id = Pubkey::try_from(&data[..32]).map_err(|e| {
                    AtrError::InvalidIntent(format!("Invalid program ID in raw data: {}", e))
                })?;
                let ix_data = &data[32..];
                instructions.push(Instruction::new_with_bytes(
                    program_id,
                    ix_data,
                    Vec::new(),
                ));
            }
            IntentOperation::Swap { .. } => {
                return Err(AtrError::InvalidIntent(
                    "Swap operations require DEX-specific instruction building (Jupiter, Raydium, etc.)".to_string(),
                ));
            }
            IntentOperation::Deploy { .. } => {
                return Err(AtrError::InvalidIntent(
                    "Contract deployment on Solana requires BPF loader instructions — use ContractCall with deploy program".to_string(),
                ));
            }
        }

        // Build unsigned transaction with a placeholder blockhash
        // The actual recent blockhash should be set right before signing
        let message = Message::new(&instructions, None);
        let transaction = Transaction::new_unsigned(message);

        Ok(transaction)
    }
}

#[async_trait]
impl Executor for SolanaExecutor {
    async fn simulate(&self, intent: &TransactionIntent) -> AtrResult<SimulationResult> {
        debug!("Simulating Solana transaction for intent {}", intent.id);

        // Build transaction
        let transaction = self.build_transaction(intent)?;

        // Simulate transaction
        match self.rpc_client.simulate_transaction(&transaction) {
            Ok(response) => {
                if let Some(err) = response.value.err {
                    warn!("Simulation failed: {:?}", err);
                    return Ok(SimulationResult {
                        success: false,
                        estimated_units: None,
                        estimated_fee: None,
                        error: Some(format!("{:?}", err)),
                        trace: None,
                    });
                }

                let units_consumed = response.value.units_consumed.unwrap_or(0);
                let estimated_fee = self.fee_estimator.estimate_priority_fee().await?;

                Ok(SimulationResult {
                    success: true,
                    estimated_units: Some(units_consumed),
                    estimated_fee: Some(estimated_fee),
                    error: None,
                    trace: response.value.logs.map(|logs| {
                        serde_json::Value::Array(
                            logs.into_iter()
                                .map(serde_json::Value::String)
                                .collect(),
                        )
                    }),
                })
            }
            Err(e) => Err(AtrError::SimulationFailed(e.to_string())),
        }
    }

    async fn submit(&self, intent: &TransactionIntent) -> AtrResult<String> {
        info!("Submitting Solana transaction for intent {}", intent.id);

        // Build transaction
        let transaction = self.build_transaction(intent)?;

        // Send transaction
        match self.rpc_client.send_transaction(&transaction) {
            Ok(signature) => {
                info!("Transaction submitted: {}", signature);
                Ok(signature.to_string())
            }
            Err(e) => Err(AtrError::SubmissionFailed(e.to_string())),
        }
    }

    async fn check_status(&self, tx_hash: &str) -> AtrResult<TransactionRecord> {
        let signature = Signature::from_str(tx_hash)
            .map_err(|e| AtrError::Internal(format!("Invalid signature: {}", e)))?;

        match self.rpc_client.get_signature_status(&signature) {
            Ok(Some(status)) => {
                let mut record = TransactionRecord::new(uuid::Uuid::new_v4(), Chain::Solana);
                record.tx_hash = Some(tx_hash.to_string());

                if let Err(e) = status {
                    record.update_status(TransactionStatus::Failed);
                    record.error = Some(format!("{:?}", e));
                } else {
                    // Check confirmation status
                    match self.rpc_client.get_transaction(
                        &signature,
                        solana_transaction_status::UiTransactionEncoding::Json,
                    ) {
                        Ok(tx) => {
                            record.update_status(TransactionStatus::Finalized);
                            // Extract slot as block number equivalent
                            record.block_number = Some(tx.slot);
                            // Extract fee from meta
                            if let Some(meta) = tx.transaction.meta {
                                record.fee_paid = Some(meta.fee);
                            }
                        }
                        Err(_) => record.update_status(TransactionStatus::Confirmed),
                    }
                }

                Ok(record)
            }
            Ok(None) => {
                let mut record = TransactionRecord::new(uuid::Uuid::new_v4(), Chain::Solana);
                record.tx_hash = Some(tx_hash.to_string());
                record.update_status(TransactionStatus::Pending);
                Ok(record)
            }
            Err(e) => Err(AtrError::RpcError(e.to_string())),
        }
    }

    async fn estimate_fee(&self) -> AtrResult<u64> {
        self.fee_estimator.estimate_priority_fee().await
    }

    async fn cancel(&self, _tx_hash: &str) -> AtrResult<String> {
        // Solana doesn't support transaction cancellation in the traditional sense
        // Once submitted, transactions either succeed or fail/expire
        Err(AtrError::Internal(
            "Transaction cancellation not supported on Solana. Transactions expire after ~60s if not confirmed.".to_string(),
        ))
    }
}
