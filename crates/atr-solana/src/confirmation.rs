//! Transaction confirmation tracking for Solana

use solana_sdk::commitment_config::CommitmentLevel;
use std::time::Duration;

/// Tracks transaction confirmation progress
pub struct ConfirmationTracker {
    target_commitment: CommitmentLevel,
    timeout: Duration,
}

impl ConfirmationTracker {
    /// Create a new confirmation tracker
    pub fn new(target_commitment: CommitmentLevel, timeout: Duration) -> Self {
        Self {
            target_commitment,
            timeout,
        }
    }

    /// Get target commitment level
    pub fn target_commitment(&self) -> CommitmentLevel {
        self.target_commitment
    }

    /// Get timeout duration
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}
