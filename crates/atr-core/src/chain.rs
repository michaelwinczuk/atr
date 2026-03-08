//! Chain identification and configuration

use serde::{Deserialize, Serialize};

/// Supported blockchain networks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Chain {
    /// Solana mainnet-beta
    Solana,
    /// Base (Ethereum L2)
    Base,
}

impl Chain {
    /// Returns the chain ID for EVM chains
    pub fn chain_id(&self) -> Option<u64> {
        match self {
            Chain::Base => Some(8453), // Base mainnet
            Chain::Solana => None,
        }
    }

    /// Returns whether this is an EVM-compatible chain
    pub fn is_evm(&self) -> bool {
        matches!(self, Chain::Base)
    }

    /// Returns whether this is Solana
    pub fn is_solana(&self) -> bool {
        matches!(self, Chain::Solana)
    }
}

impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Chain::Solana => write!(f, "solana"),
            Chain::Base => write!(f, "base"),
        }
    }
}
