//! Error types for ATR

use thiserror::Error;

/// Core ATR error type
#[derive(Error, Debug)]
pub enum AtrError {
    #[error("Invalid intent: {0}")]
    InvalidIntent(String),

    #[error("Chain not supported: {0}")]
    UnsupportedChain(String),

    #[error("Simulation failed: {0}")]
    SimulationFailed(String),

    #[error("Transaction submission failed: {0}")]
    SubmissionFailed(String),

    #[error("Transaction confirmation timeout")]
    ConfirmationTimeout,

    #[error("Transaction failed on-chain: {0}")]
    OnChainFailure(String),

    #[error("Retry limit exceeded")]
    RetryLimitExceeded,

    #[error("Idempotency key conflict: {0}")]
    IdempotencyConflict(String),

    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type AtrResult<T> = Result<T, AtrError>;
