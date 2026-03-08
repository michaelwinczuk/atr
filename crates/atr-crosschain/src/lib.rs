//! Cross-chain transaction coordination

use atr_core::{
    intent::{IntentBatch, TransactionIntent},
    transaction::TransactionRecord,
    error::{AtrError, AtrResult},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
        let pair = self.pairs.get(&pair_id)
            .ok_or_else(|| AtrError::Internal("Pair not found".to_string()))?;

        let tx_a_record = records.iter().find(|r| r.id == pair.tx_a.id);
        let tx_b_record = records.iter().find(|r| r.id == pair.tx_b.id);

        match (tx_a_record, tx_b_record) {
            (Some(a), Some(b)) => {
                if a.is_terminal() && b.is_terminal() {
                    Ok(PairStatus::Completed)
                } else {
                    Ok(PairStatus::InProgress)
                }
            }
            _ => Ok(PairStatus::Pending),
        }
    }

    /// Handle rollback when one transaction fails
    pub async fn handle_rollback(&self, pair_id: Uuid, failed_tx: Uuid) -> AtrResult<()> {
        warn!("Handling rollback for pair {} (failed tx: {})", pair_id, failed_tx);

        // TODO [Phase 2]: Implement rollback logic
        // - Identify the successful transaction
        // - Submit compensating transaction
        // - Track rollback status

        Ok(())
    }
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
    /// Both transactions completed
    Completed,
}
