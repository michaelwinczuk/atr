//! Solana transaction executor

use async_trait::async_trait;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
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
    keypair: Option<Keypair>,
}

impl SolanaExecutor {
    /// Create a new Solana executor
    pub fn new(rpc_url: String, commitment: CommitmentConfig) -> Self {
        let rpc_client = RpcClient::new_with_commitment(rpc_url.clone(), commitment);
        let fee_estimator = PriorityFeeEstimator::new(rpc_url);
        Self {
            rpc_client,
            fee_estimator,
            keypair: None,
        }
    }

    /// Set the signing keypair
    pub fn with_keypair(mut self, keypair: Keypair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    /// Set keypair from base58-encoded private key
    pub fn with_keypair_base58(mut self, base58_key: &str) -> AtrResult<Self> {
        let bytes = bs58::decode(base58_key)
            .into_vec()
            .map_err(|e| AtrError::ConfigError(format!("Invalid base58 key: {}", e)))?;
        let keypair = Keypair::from_bytes(&bytes)
            .map_err(|e| AtrError::ConfigError(format!("Invalid keypair bytes: {}", e)))?;
        self.keypair = Some(keypair);
        Ok(self)
    }

    /// Get the fee payer pubkey
    fn fee_payer(&self) -> AtrResult<Pubkey> {
        self.keypair
            .as_ref()
            .map(|kp| kp.pubkey())
            .ok_or_else(|| AtrError::ConfigError("Keypair not configured".to_string()))
    }

    /// Build a Solana transaction from intent
    fn build_transaction(&self, intent: &TransactionIntent) -> AtrResult<Transaction> {
        let fee_payer = self.fee_payer()?;
        let mut instructions = Vec::new();

        // Add compute budget instructions for priority fees
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(200_000));
        instructions.push(ComputeBudgetInstruction::set_compute_unit_price(5000));

        match &intent.operation {
            IntentOperation::Transfer { to, amount } => {
                let to_pubkey = Pubkey::from_str(to).map_err(|e| {
                    AtrError::InvalidIntent(format!("Invalid Solana address '{}': {}", to, e))
                })?;
                instructions.push(system_instruction::transfer(
                    &fee_payer,
                    &to_pubkey,
                    *amount,
                ));
            }
            IntentOperation::ContractCall {
                contract,
                args,
                ..
            } => {
                let program_id = Pubkey::from_str(contract).map_err(|e| {
                    AtrError::InvalidIntent(format!("Invalid program address: {}", e))
                })?;

                let data = if let Some(data_str) = args.get("data").and_then(|d| d.as_str()) {
                    hex::decode(data_str.trim_start_matches("0x")).map_err(|e| {
                        AtrError::InvalidIntent(format!("Invalid instruction data: {}", e))
                    })?
                } else {
                    Vec::new()
                };

                let accounts: Vec<AccountMeta> =
                    if let Some(accts) = args.get("accounts").and_then(|a| a.as_array()) {
                        accts
                            .iter()
                            .filter_map(|a| {
                                let pubkey = Pubkey::from_str(a.get("pubkey")?.as_str()?).ok()?;
                                let is_signer = a
                                    .get("isSigner")
                                    .and_then(|s| s.as_bool())
                                    .unwrap_or(false);
                                let is_writable = a
                                    .get("isWritable")
                                    .and_then(|w| w.as_bool())
                                    .unwrap_or(false);
                                Some(if is_writable {
                                    AccountMeta::new(pubkey, is_signer)
                                } else {
                                    AccountMeta::new_readonly(pubkey, is_signer)
                                })
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };

                instructions.push(Instruction::new_with_bytes(program_id, &data, accounts));
            }
            IntentOperation::Raw { data } => {
                if data.len() < 32 {
                    return Err(AtrError::InvalidIntent(
                        "Raw data must contain at least 32 bytes for program ID".to_string(),
                    ));
                }
                let program_id = Pubkey::try_from(&data[..32]).map_err(|e| {
                    AtrError::InvalidIntent(format!("Invalid program ID: {}", e))
                })?;
                instructions.push(Instruction::new_with_bytes(
                    program_id,
                    &data[32..],
                    Vec::new(),
                ));
            }
            IntentOperation::Swap { .. } => {
                return Err(AtrError::InvalidIntent(
                    "Swap operations require DEX-specific instruction building".to_string(),
                ));
            }
            IntentOperation::Deploy { .. } => {
                return Err(AtrError::InvalidIntent(
                    "Use ContractCall with BPF loader for Solana deployments".to_string(),
                ));
            }
        }

        let message = Message::new(&instructions, Some(&fee_payer));
        Ok(Transaction::new_unsigned(message))
    }
}

#[async_trait]
impl Executor for SolanaExecutor {
    async fn simulate(&self, intent: &TransactionIntent) -> AtrResult<SimulationResult> {
        debug!("Simulating Solana transaction for intent {}", intent.id);

        let transaction = self.build_transaction(intent)?;

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

        let keypair = self
            .keypair
            .as_ref()
            .ok_or_else(|| AtrError::ConfigError("Keypair not configured for signing".to_string()))?;

        let mut transaction = self.build_transaction(intent)?;

        // Get recent blockhash for transaction validity
        let blockhash = self
            .rpc_client
            .get_latest_blockhash()
            .map_err(|e| AtrError::RpcError(format!("Failed to get blockhash: {}", e)))?;

        // Sign the transaction
        transaction
            .try_sign(&[keypair], blockhash)
            .map_err(|e| AtrError::Internal(format!("Signing failed: {}", e)))?;

        // Send
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
                    match self.rpc_client.get_transaction(
                        &signature,
                        solana_transaction_status::UiTransactionEncoding::Json,
                    ) {
                        Ok(tx) => {
                            record.update_status(TransactionStatus::Finalized);
                            record.block_number = Some(tx.slot);
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
        Err(AtrError::Internal(
            "Solana transactions cannot be cancelled. They expire after ~60s if not confirmed."
                .to_string(),
        ))
    }
}
