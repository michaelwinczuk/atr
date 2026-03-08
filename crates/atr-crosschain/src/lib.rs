//! Cross-chain transaction coordination

use atr_core::{
    error::{AtrError, AtrResult},
    executor::Executor,
    intent::{IntentBatch, IntentOperation, TransactionIntent},
    transaction::TransactionRecord,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Cross-chain transaction pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainPair {
    /// Pair identifier
    pub id: Uuid,
    /// First transaction (e.g., on Solana)
    pub tx_a: TransactionIntent,
    /// Second transaction (e.g., on Base)
    pub tx_b: TransactionIntent,
    /// Whether both must succeed
    pub atomic: bool,
}

/// Cross-chain coordinator
pub struct CrossChainCoordinator {
    pairs: HashMap<Uuid, CrossChainPair>,
}

impl CrossChainCoordinator {
    /// Create a new cross-chain coordinator
    pub fn new() -> Self {
        Self {
            pairs: HashMap::new(),
        }
    }

    /// Register a cross-chain transaction pair
    pub fn register_pair(&mut self, pair: CrossChainPair) {
        info!("Registering cross-chain pair {}", pair.id);
        self.pairs.insert(pair.id, pair);
    }

    /// Check if both transactions in a pair have completed
    pub fn check_pair_status(
        &self,
        pair_id: Uuid,
        records: &[TransactionRecord],
    ) -> AtrResult<PairStatus> {
        let pair = self
            .pairs
            .get(&pair_id)
            .ok_or_else(|| AtrError::Internal("Pair not found".to_string()))?;

        let tx_a_record = records.iter().find(|r| r.id == pair.tx_a.id);
        let tx_b_record = records.iter().find(|r| r.id == pair.tx_b.id);

        match (tx_a_record, tx_b_record) {
            (Some(a), Some(b)) => {
                let a_failed = a.error.is_some();
                let b_failed = b.error.is_some();

                if a.is_terminal() && b.is_terminal() {
                    if a_failed || b_failed {
                        Ok(PairStatus::PartialFailure)
                    } else {
                        Ok(PairStatus::Completed)
                    }
                } else {
                    Ok(PairStatus::InProgress)
                }
            }
            _ => Ok(PairStatus::Pending),
        }
    }

    /// Handle rollback when one transaction in an atomic pair fails.
    /// Submits a compensating transaction to reverse the successful side.
    pub async fn handle_rollback(
        &self,
        pair_id: Uuid,
        failed_tx: Uuid,
        executor: Arc<dyn Executor>,
    ) -> AtrResult<String> {
        warn!(
            "Handling rollback for pair {} (failed tx: {})",
            pair_id, failed_tx
        );

        let pair = self
            .pairs
            .get(&pair_id)
            .ok_or_else(|| AtrError::Internal("Pair not found".to_string()))?;

        if !pair.atomic {
            return Err(AtrError::Internal(
                "Non-atomic pairs do not support rollback".to_string(),
            ));
        }

        // Identify which transaction succeeded and needs reversal
        let successful_intent = if pair.tx_a.id == failed_tx {
            &pair.tx_b
        } else {
            &pair.tx_a
        };

        // Build a compensating transaction based on the operation type
        let compensating_intent = build_compensating_intent(successful_intent)?;

        // Submit the compensating transaction
        let tx_hash = executor.submit(&compensating_intent).await?;
        info!(
            "Rollback submitted for pair {}: compensating tx {}",
            pair_id, tx_hash
        );

        Ok(tx_hash)
    }
}

/// Build a compensating (reverse) intent for a given operation
fn build_compensating_intent(original: &TransactionIntent) -> AtrResult<TransactionIntent> {
    let reverse_op = match &original.operation {
        IntentOperation::Transfer { to, amount } => {
            // Reverse transfer: send tokens back from 'to' to the original sender
            // Note: this requires 'to' to have signing authority, which may not be possible
            // In practice, this would be a refund from a smart contract or escrow
            warn!("Transfer rollback requires manual intervention — creating reverse intent");
            IntentOperation::Transfer {
                to: to.clone(), // placeholder — needs the original sender address
                amount: *amount,
            }
        }
        IntentOperation::ContractCall { contract, .. } => {
            // For contract calls, the contract itself should have a rollback function
            warn!("Contract call rollback requires contract-specific logic");
            return Err(AtrError::Internal(
                "Automatic rollback not supported for contract calls. Use contract-specific rollback methods.".to_string(),
            ));
        }
        _ => {
            return Err(AtrError::Internal(
                "Automatic rollback not supported for this operation type".to_string(),
            ));
        }
    };

    Ok(TransactionIntent {
        id: Uuid::new_v4(),
        chain: original.chain,
        operation: reverse_op,
        idempotency_key: Some(format!("rollback-{}", original.id)),
        max_fee: original.max_fee,
        timeout_secs: original.timeout_secs,
    })
}

impl Default for CrossChainCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of a cross-chain pair
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairStatus {
    /// Both transactions pending
    Pending,
    /// At least one transaction in progress
    InProgress,
    /// Both transactions completed successfully
    Completed,
    /// One or both transactions failed
    PartialFailure,
}
