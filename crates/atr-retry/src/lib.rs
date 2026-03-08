//! Retry engine with configurable policies

use atr_core::error::{AtrError, AtrResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

/// Retry policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Whether to add jitter to backoff
    pub use_jitter: bool,
    /// Fee escalation percentage per retry (0.0-1.0)
    pub fee_escalation: f64,
    /// Timeout for entire retry sequence
    pub timeout: Option<Duration>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            use_jitter: true,
            fee_escalation: 0.1, // 10% increase per retry
            timeout: Some(Duration::from_secs(300)), // 5 minutes
        }
    }
}

impl RetryPolicy {
    /// Calculate backoff duration for a given attempt
    pub fn backoff_duration(&self, attempt: u32) -> Duration {
        let base = self.initial_backoff.as_secs_f64()
            * self.backoff_multiplier.powi(attempt as i32);
        let capped = base.min(self.max_backoff.as_secs_f64());

        let duration = if self.use_jitter {
            // Add up to 25% jitter
            let jitter = capped * (rand::random::<f64>() * 0.25);
            Duration::from_secs_f64(capped + jitter)
        } else {
            Duration::from_secs_f64(capped)
        };

        duration
    }

    /// Calculate escalated fee for a given attempt
    pub fn escalated_fee(&self, base_fee: u64, attempt: u32) -> u64 {
        let multiplier = 1.0 + (self.fee_escalation * attempt as f64);
        (base_fee as f64 * multiplier) as u64
    }

    /// Check if should retry based on attempt count
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

/// Retry engine for executing operations with retry logic
pub struct RetryEngine {
    policy: RetryPolicy,
}

impl RetryEngine {
    /// Create a new retry engine with the given policy
    pub fn new(policy: RetryPolicy) -> Self {
        Self { policy }
    }

    /// Execute an operation with retry logic
    pub async fn execute<F, Fut, T>(&self, operation: F) -> AtrResult<T>
    where
        F: Fn(u32) -> Fut,
        Fut: std::future::Future<Output = AtrResult<T>>,
    {
        let mut attempt = 0;

        loop {
            debug!("Retry attempt {}/{}", attempt + 1, self.policy.max_attempts);

            match operation(attempt).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempt += 1;

                    if !self.policy.should_retry(attempt) {
                        warn!("Retry limit exceeded after {} attempts", attempt);
                        return Err(AtrError::RetryLimitExceeded);
                    }

                    let backoff = self.policy.backoff_duration(attempt);
                    debug!("Retrying after {:?}, error: {}", backoff, e);
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }
}

// Mock rand::random for compilation
mod rand {
    pub fn random<T>() -> T
    where
        T: Default,
    {
        T::default()
    }
}
