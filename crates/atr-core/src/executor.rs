//! Executor trait for chain-specific transaction execution

use async_trait::async_trait;

use crate::{
    intent::TransactionIntent,
    transaction::{SimulationResult, TransactionRecord},
    error::AtrResult,
};

/// Chain-specific transaction executor
#[async_trait]
pub trait Executor: Send + Sync {
    /// Simulate transaction before submission
    async fn simulate(&self, intent: &TransactionIntent) -> AtrResult<SimulationResult>;

    /// Submit transaction to the network
    async fn submit(&self, intent: &TransactionIntent) -> AtrResult<String>;

    /// Check transaction status
    async fn check_status(&self, tx_hash: &str) -> AtrResult<TransactionRecord>;

    /// Estimate optimal fee for current network conditions
    async fn estimate_fee(&self) -> AtrResult<u64>;

    /// Cancel or replace a pending transaction
    async fn cancel(&self, tx_hash: &str) -> AtrResult<String>;
}
