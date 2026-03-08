//! Solana transaction executor

use async_trait::async_trait;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::Signature,
    transaction::Transaction,
};
use std::str::FromStr;
use tracing::{debug, info, warn};

use atr_core::{
    executor::Executor,
    intent::TransactionIntent,
    transaction::{SimulationResult, TransactionRecord, TransactionStatus},
    error::{AtrError, AtrResult},
    chain::Chain,
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
    fn build_transaction(&self, _intent: &TransactionIntent) -> AtrResult<Transaction> {
        // TODO [Phase 2]: Implement actual transaction building from intent
        // For now, return error indicating not implemented
        Err(AtrError::Internal(
            "Transaction building not yet implemented".to_string(),
        ))
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
                    trace: None,
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
                let mut record = TransactionRecord::new(
                    uuid::Uuid::new_v4(),
                    Chain::Solana,
                );
                record.tx_hash = Some(tx_hash.to_string());

                if let Err(e) = status {
                    record.update_status(TransactionStatus::Failed);
                    record.error = Some(format!("{:?}", e));
                } else {
                    // Check confirmation status
                    match self.rpc_client.get_transaction(&signature, solana_transaction_status::UiTransactionEncoding::Json) {
                        Ok(_) => record.update_status(TransactionStatus::Finalized),
                        Err(_) => record.update_status(TransactionStatus::Confirmed),
                    }
                }

                Ok(record)
            }
            Ok(None) => {
                let mut record = TransactionRecord::new(
                    uuid::Uuid::new_v4(),
                    Chain::Solana,
                );
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
        // Once submitted, transactions either succeed or fail
        Err(AtrError::Internal(
            "Transaction cancellation not supported on Solana".to_string(),
        ))
    }
}
