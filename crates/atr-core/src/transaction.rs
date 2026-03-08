//! Transaction lifecycle and status tracking

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::chain::Chain;

/// Transaction lifecycle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    /// Intent received, not yet processed
    Pending,
    /// Pre-flight simulation in progress
    Simulating,
    /// Simulation completed successfully
    Simulated,
    /// Simulation failed
    SimulationFailed,
    /// Transaction submitted to network
    Submitted,
    /// Transaction confirmed (1+ confirmations)
    Confirmed,
    /// Transaction finalized (irreversible)
    Finalized,
    /// Transaction failed on-chain
    Failed,
    /// Transaction dropped/expired
    Dropped,
    /// Retry in progress
    Retrying,
}

/// Transaction record with full lifecycle data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    /// Transaction identifier (maps to intent ID)
    pub id: Uuid,
    /// Target chain
    pub chain: Chain,
    /// Current status
    pub status: TransactionStatus,
    /// Transaction hash (once submitted)
    pub tx_hash: Option<String>,
    /// Block number (once confirmed)
    pub block_number: Option<u64>,
    /// Gas/compute units used
    pub units_used: Option<u64>,
    /// Actual fee paid (in native token units)
    pub fee_paid: Option<u64>,
    /// Error message if failed
    pub error: Option<String>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Finalization timestamp
    pub finalized_at: Option<DateTime<Utc>>,
}

impl TransactionRecord {
    /// Create a new pending transaction record
    pub fn new(id: Uuid, chain: Chain) -> Self {
        let now = Utc::now();
        Self {
            id,
            chain,
            status: TransactionStatus::Pending,
            tx_hash: None,
            block_number: None,
            units_used: None,
            fee_paid: None,
            error: None,
            retry_count: 0,
            created_at: now,
            updated_at: now,
            finalized_at: None,
        }
    }

    /// Update status and timestamp
    pub fn update_status(&mut self, status: TransactionStatus) {
        self.status = status;
        self.updated_at = Utc::now();
        if matches!(status, TransactionStatus::Finalized | TransactionStatus::Failed) {
            self.finalized_at = Some(Utc::now());
        }
    }

    /// Check if transaction is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            TransactionStatus::Finalized
                | TransactionStatus::Failed
                | TransactionStatus::Dropped
                | TransactionStatus::SimulationFailed
        )
    }
}

/// Simulation result from pre-flight checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    /// Whether simulation succeeded
    pub success: bool,
    /// Estimated gas/compute units
    pub estimated_units: Option<u64>,
    /// Estimated fee
    pub estimated_fee: Option<u64>,
    /// Error message if simulation failed
    pub error: Option<String>,
    /// Execution trace (for debugging)
    pub trace: Option<serde_json::Value>,
}
