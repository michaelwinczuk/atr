//! Application state for the server

use atr_observer::{MetricsCollector, TransactionTracker};
use atr_crosschain::CrossChainCoordinator;
use std::sync::Arc;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub metrics: Arc<MetricsCollector>,
    pub tracker: Arc<TransactionTracker>,
    pub coordinator: Arc<tokio::sync::Mutex<CrossChainCoordinator>>,
}

impl AppState {
    /// Create new application state
    pub fn new() -> Self {
        let metrics = Arc::new(MetricsCollector::new());
        let tracker = Arc::new(TransactionTracker::new(metrics.clone()));
        let coordinator = Arc::new(tokio::sync::Mutex::new(CrossChainCoordinator::new()));

        Self {
            metrics,
            tracker,
            coordinator,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
