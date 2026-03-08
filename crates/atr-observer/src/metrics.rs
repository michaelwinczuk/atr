//! Prometheus metrics collection

use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::Registry;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Snapshot of current metrics values
pub struct MetricsSnapshot {
    pub submissions: u64,
    pub confirmations: u64,
    pub failures: u64,
    pub avg_confirmation_time: f64,
}

/// Metrics collector for ATR operations
pub struct MetricsCollector {
    registry: Arc<Registry>,
    transactions_submitted: Counter,
    transactions_confirmed: Counter,
    transactions_failed: Counter,
    confirmation_time: Histogram,
    // Atomic counters for snapshot access
    submit_count: AtomicU64,
    confirm_count: AtomicU64,
    fail_count: AtomicU64,
    total_confirm_time: AtomicU64, // stored as milliseconds
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        let mut registry = Registry::default();

        let transactions_submitted = Counter::default();
        let transactions_confirmed = Counter::default();
        let transactions_failed = Counter::default();
        let confirmation_time =
            Histogram::new(vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0].into_iter());

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
            submit_count: AtomicU64::new(0),
            confirm_count: AtomicU64::new(0),
            fail_count: AtomicU64::new(0),
            total_confirm_time: AtomicU64::new(0),
        }
    }

    /// Record a transaction submission
    pub fn record_submission(&self) {
        self.transactions_submitted.inc();
        self.submit_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a transaction confirmation
    pub fn record_confirmation(&self, duration_secs: f64) {
        self.transactions_confirmed.inc();
        self.confirmation_time.observe(duration_secs);
        self.confirm_count.fetch_add(1, Ordering::Relaxed);
        self.total_confirm_time
            .fetch_add((duration_secs * 1000.0) as u64, Ordering::Relaxed);
    }

    /// Record a transaction failure
    pub fn record_failure(&self) {
        self.transactions_failed.inc();
        self.fail_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        let confirmations = self.confirm_count.load(Ordering::Relaxed);
        let total_time_ms = self.total_confirm_time.load(Ordering::Relaxed);
        let avg_time = if confirmations > 0 {
            (total_time_ms as f64 / 1000.0) / confirmations as f64
        } else {
            0.0
        };

        MetricsSnapshot {
            submissions: self.submit_count.load(Ordering::Relaxed),
            confirmations,
            failures: self.fail_count.load(Ordering::Relaxed),
            avg_confirmation_time: avg_time,
        }
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
