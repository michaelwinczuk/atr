//! Prometheus metrics collection

use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::Registry;
use std::sync::Arc;

/// Metrics collector for ATR operations
pub struct MetricsCollector {
    registry: Arc<Registry>,
    transactions_submitted: Counter,
    transactions_confirmed: Counter,
    transactions_failed: Counter,
    confirmation_time: Histogram,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        let mut registry = Registry::default();

        let transactions_submitted = Counter::default();
        let transactions_confirmed = Counter::default();
        let transactions_failed = Counter::default();
        let confirmation_time = Histogram::new(vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0].into_iter());

        registry.register(
            "atr_transactions_submitted",
            "Total transactions submitted",
            transactions_submitted.clone(),
        );
        registry.register(
            "atr_transactions_confirmed",
            "Total transactions confirmed",
            transactions_confirmed.clone(),
        );
        registry.register(
            "atr_transactions_failed",
            "Total transactions failed",
            transactions_failed.clone(),
        );
        registry.register(
            "atr_confirmation_time_seconds",
            "Transaction confirmation time",
            confirmation_time.clone(),
        );

        Self {
            registry: Arc::new(registry),
            transactions_submitted,
            transactions_confirmed,
            transactions_failed,
            confirmation_time,
        }
    }

    /// Record a transaction submission
    pub fn record_submission(&self) {
        self.transactions_submitted.inc();
    }

    /// Record a transaction confirmation
    pub fn record_confirmation(&self, duration_secs: f64) {
        self.transactions_confirmed.inc();
        self.confirmation_time.observe(duration_secs);
    }

    /// Record a transaction failure
    pub fn record_failure(&self) {
        self.transactions_failed.inc();
    }

    /// Get the metrics registry
    pub fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
