//! Pre-flight transaction simulation

use atr_core::{
    intent::TransactionIntent,
    transaction::SimulationResult,
    error::AtrResult,
    executor::Executor,
    chain::Chain,
};
#[cfg(feature = "solana")]
use atr_solana::SolanaExecutor;
use atr_base::BaseExecutor;
use tracing::info;

/// Transaction simulator that routes to chain-specific simulators
pub struct TransactionSimulator {
    #[cfg(feature = "solana")]
    solana_executor: Option<SolanaExecutor>,
    base_executor: Option<BaseExecutor>,
}

impl TransactionSimulator {
    /// Create a new transaction simulator
    pub fn new(
        #[cfg(feature = "solana")] solana_executor: Option<SolanaExecutor>,
        base_executor: Option<BaseExecutor>,
    ) -> Self {
        Self {
            #[cfg(feature = "solana")]
            solana_executor,
            base_executor,
        }
    }

    /// Simulate a transaction intent
    pub async fn simulate(&self, intent: &TransactionIntent) -> AtrResult<SimulationResult> {
        info!("Simulating transaction for intent {} on {}", intent.id, intent.chain);

        match intent.chain {
            Chain::Solana => {
                #[cfg(feature = "solana")]
                if let Some(executor) = &self.solana_executor {
                    return executor.simulate(intent).await;
                }
                Err(atr_core::error::AtrError::ConfigError(
                    "Solana executor not configured".to_string(),
                ))
            }
            Chain::Base | Chain::Shape => {
                if let Some(executor) = &self.base_executor {
                    executor.simulate(intent).await
                } else {
                    Err(atr_core::error::AtrError::ConfigError(
                        format!("{} executor not configured", intent.chain),
                    ))
                }
            }
        }
    }
}
