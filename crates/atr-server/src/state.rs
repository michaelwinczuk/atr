//! Application state for the server

use atr_core::chain::Chain;
use atr_core::executor::Executor;
use atr_crosschain::CrossChainCoordinator;
use atr_observer::{MetricsCollector, TransactionTracker};
use atr_base::BaseExecutor;
use atr_solana::SolanaExecutor;
use solana_sdk::commitment_config::CommitmentConfig;
use std::sync::Arc;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub metrics: Arc<MetricsCollector>,
    pub tracker: Arc<TransactionTracker>,
    pub coordinator: Arc<tokio::sync::Mutex<CrossChainCoordinator>>,
    pub solana_executor: Option<Arc<SolanaExecutor>>,
    pub base_executor: Option<Arc<BaseExecutor>>,
}

impl AppState {
    /// Create new application state
    pub fn new() -> Self {
        let metrics = Arc::new(MetricsCollector::new());
        let tracker = Arc::new(TransactionTracker::new(metrics.clone()));
        let coordinator = Arc::new(tokio::sync::Mutex::new(CrossChainCoordinator::new()));

        // Initialize executors from environment variables
        let solana_executor = std::env::var("SOLANA_RPC_URL").ok().map(|url| {
            Arc::new(SolanaExecutor::new(url, CommitmentConfig::confirmed()))
        });

        let base_executor = std::env::var("BASE_RPC_URL").ok().map(|url| {
            let executor = BaseExecutor::new(url);
            // Optionally set sender address
            let executor = if let Ok(addr) = std::env::var("BASE_SENDER_ADDRESS") {
                executor.with_sender(addr)
            } else {
                executor
            };
            Arc::new(executor)
        });

        Self {
            metrics,
            tracker,
            coordinator,
            solana_executor,
            base_executor,
        }
    }

    /// Get the executor for a given chain
    pub fn get_executor(&self, chain: Chain) -> Option<Arc<dyn Executor>> {
        match chain {
            Chain::Solana => self.solana_executor.clone().map(|e| e as Arc<dyn Executor>),
            Chain::Base => self.base_executor.clone().map(|e| e as Arc<dyn Executor>),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
