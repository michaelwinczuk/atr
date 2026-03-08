//! Base transaction executor

use async_trait::async_trait;
use tracing::{debug, info};

use atr_core::{
    executor::Executor,
    intent::TransactionIntent,
    transaction::{SimulationResult, TransactionRecord, TransactionStatus},
    error::{AtrError, AtrResult},
    chain::Chain,
};

use crate::gas_estimator::GasEstimator;
use crate::nonce_manager::NonceManager;

/// Base-specific transaction executor
pub struct BaseExecutor {
    rpc_url: String,
    gas_estimator: GasEstimator,
    nonce_manager: NonceManager,
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
        }
    }
}

#[async_trait]
impl Executor for BaseExecutor {
    async fn simulate(&self, intent: &TransactionIntent) -> AtrResult<SimulationResult> {
        debug!("Simulating Base transaction for intent {}", intent.id);

        // TODO [Phase 2]: Implement actual simulation using eth_call or trace_call
        // - Build transaction from intent
        // - Call eth_estimateGas
        // - Simulate execution with trace_call
        // - Return gas estimates and execution trace

        // Phase 1: Return safe estimates
        let estimated_gas = self.gas_estimator.estimate_gas().await?;
        let estimated_fee = estimated_gas * 1_000_000_000; // 1 gwei base fee estimate

        Ok(SimulationResult {
            success: true,
            estimated_units: Some(estimated_gas),
            estimated_fee: Some(estimated_fee),
            error: None,
            trace: None,
        })
    }

    async fn submit(&self, intent: &TransactionIntent) -> AtrResult<String> {
        info!("Submitting Base transaction for intent {}", intent.id);

        // TODO [Phase 2]: Implement actual transaction submission
        // - Build EIP-1559 transaction from intent
        // - Get nonce from nonce manager
        // - Sign transaction (agent provides signer)
        // - Submit via eth_sendRawTransaction
        // - Return transaction hash

        Err(AtrError::Internal(
            "Transaction submission not yet implemented".to_string(),
        ))
    }

    async fn check_status(&self, tx_hash: &str) -> AtrResult<TransactionRecord> {
        debug!("Checking status for Base transaction {}", tx_hash);

        // TODO [Phase 2]: Implement status checking
        // - Call eth_getTransactionReceipt
        // - Parse receipt status
        // - Get block number and confirmations
        // - Return transaction record

        let mut record = TransactionRecord::new(uuid::Uuid::new_v4(), Chain::Base);
        record.tx_hash = Some(tx_hash.to_string());
        record.update_status(TransactionStatus::Pending);
        Ok(record)
    }

    async fn estimate_fee(&self) -> AtrResult<u64> {
        let gas = self.gas_estimator.estimate_gas().await?;
        let base_fee = self.gas_estimator.estimate_base_fee().await?;
        Ok(gas * base_fee)
    }

    async fn cancel(&self, tx_hash: &str) -> AtrResult<String> {
        info!("Cancelling Base transaction {}", tx_hash);

        // TODO [Phase 2]: Implement transaction cancellation
        // - Get original transaction nonce
        // - Submit replacement transaction with higher gas and same nonce
        // - Return new transaction hash

        Err(AtrError::Internal(
            "Transaction cancellation not yet implemented".to_string(),
        ))
    }
}
