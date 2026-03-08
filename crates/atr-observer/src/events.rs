//! Observability events for structured logging

use atr_core::transaction::TransactionStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Structured event for observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityEvent {
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Transaction ID
    pub transaction_id: Uuid,
    /// Event type
    pub event_type: EventType,
    /// Transaction status
    pub status: TransactionStatus,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

/// Types of observability events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Intent received
    IntentReceived,
    /// Simulation started
    SimulationStarted,
    /// Simulation completed
    SimulationCompleted,
    /// Transaction submitted
    TransactionSubmitted,
    /// Status check performed
    StatusChecked,
    /// Transaction confirmed
    TransactionConfirmed,
    /// Transaction finalized
    TransactionFinalized,
    /// Transaction failed
    TransactionFailed,
    /// Retry attempted
    RetryAttempted,
}

impl ObservabilityEvent {
    /// Create a new event
    pub fn new(
        transaction_id: Uuid,
        event_type: EventType,
        status: TransactionStatus,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            transaction_id,
            event_type,
            status,
            metadata,
        }
    }
}
