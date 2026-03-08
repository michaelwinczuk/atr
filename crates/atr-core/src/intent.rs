//! Transaction intent definitions

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::chain::Chain;

/// High-level transaction intent submitted by agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionIntent {
    /// Unique intent identifier
    pub id: Uuid,
    /// Target blockchain
    pub chain: Chain,
    /// Intent operation
    pub operation: IntentOperation,
    /// Idempotency key to prevent duplicate execution
    pub idempotency_key: Option<String>,
    /// Maximum fee willing to pay (in native token units)
    pub max_fee: Option<u64>,
    /// Timeout in seconds
    pub timeout_secs: Option<u64>,
}

/// Supported intent operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IntentOperation {
    /// Transfer native tokens
    Transfer {
        to: String,
        amount: u64,
    },
    /// Swap tokens via DEX
    Swap {
        from_token: String,
        to_token: String,
        amount_in: u64,
        min_amount_out: u64,
        dex: String,
    },
    /// Call a smart contract
    ContractCall {
        contract: String,
        method: String,
        args: serde_json::Value,
        value: Option<u64>,
    },
    /// Deploy a smart contract (EVM only)
    Deploy {
        bytecode: String,
        constructor_args: Option<serde_json::Value>,
    },
    /// Execute a raw transaction
    Raw {
        data: Vec<u8>,
    },
}

/// Batch of intents to execute atomically or sequentially
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentBatch {
    /// Batch identifier
    pub id: Uuid,
    /// Intents in execution order
    pub intents: Vec<TransactionIntent>,
    /// Execution mode
    pub mode: BatchMode,
}

/// Batch execution mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchMode {
    /// Execute all intents, stop on first failure
    Sequential,
    /// Execute all intents, continue on failure
    BestEffort,
    /// Execute only if all intents can be simulated successfully
    Atomic,
}
