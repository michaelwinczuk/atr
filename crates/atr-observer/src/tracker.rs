//! Transaction lifecycle tracker

use atr_core::transaction::{TransactionRecord, TransactionStatus};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::info;
use uuid::Uuid;

use crate::events::{EventType, ObservabilityEvent};
use crate::metrics::MetricsCollector;

/// Tracks transaction lifecycle and emits events
pub struct TransactionTracker {
    transactions: Arc<RwLock<HashMap<Uuid, TransactionRecord>>>,
    metrics: Arc<MetricsCollector>,
}

impl TransactionTracker {
    /// Create a new transaction tracker
    pub fn new(metrics: Arc<MetricsCollector>) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            metrics,
        }
    }

    /// Track a new transaction
    pub fn track(&self, record: TransactionRecord) {
        let mut txs = self.transactions.write().unwrap();
        txs.insert(record.id, record);
    }

    /// Update transaction status
    pub fn update_status(&self, id: Uuid, status: TransactionStatus) {
        let mut txs = self.transactions.write().unwrap();
        if let Some(record) = txs.get_mut(&id) {
            let old_status = record.status;
            record.update_status(status);

            // Emit event
            let event_type = match status {
                TransactionStatus::Submitted => EventType::TransactionSubmitted,
                TransactionStatus::Confirmed => EventType::TransactionConfirmed,
                TransactionStatus::Finalized => EventType::TransactionFinalized,
                TransactionStatus::Failed => EventType::TransactionFailed,
                _ => EventType::StatusChecked,
            };

            let event = ObservabilityEvent::new(
                id,
                event_type,
                status,
                serde_json::json!({
                    "old_status": old_status,
                    "new_status": status,
                }),
            );

            info!("Transaction {} status: {:?} -> {:?}", id, old_status, status);

            // Update metrics
            match status {
                TransactionStatus::Submitted => self.metrics.record_submission(),
                TransactionStatus::Finalized => {
                    let duration = (record.finalized_at.unwrap() - record.created_at)
                        .num_seconds() as f64;
                    self.metrics.record_confirmation(duration);
                }
                TransactionStatus::Failed => self.metrics.record_failure(),
                _ => {}
            }
        }
    }

    /// Get transaction record
    pub fn get(&self, id: Uuid) -> Option<TransactionRecord> {
        let txs = self.transactions.read().unwrap();
        txs.get(&id).cloned()
    }

    /// Get all transactions
    pub fn get_all(&self) -> Vec<TransactionRecord> {
        let txs = self.transactions.read().unwrap();
        txs.values().cloned().collect()
    }
}
