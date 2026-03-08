//! Application state for the server

use atr_base::BaseExecutor;
use atr_core::chain::Chain;
use atr_core::executor::Executor;
use atr_crosschain::CrossChainCoordinator;
use atr_observer::{MetricsCollector, Storage, TransactionTracker};
#[cfg(feature = "solana")]
use atr_solana::SolanaExecutor;
#[cfg(feature = "solana")]
use solana_sdk::commitment_config::CommitmentConfig;
use std::sync::Arc;
use tracing::{info, warn};

use crate::rate_limit::RateLimiter;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub metrics: Arc<MetricsCollector>,
    pub tracker: Arc<TransactionTracker>,
    pub coordinator: Arc<tokio::sync::Mutex<CrossChainCoordinator>>,
    #[cfg(feature = "solana")]
    pub solana_executor: Option<Arc<SolanaExecutor>>,
    pub base_executor: Option<Arc<BaseExecutor>>,
    pub shape_executor: Option<Arc<BaseExecutor>>,
    pub storage: Arc<Storage>,
    pub rate_limiter: RateLimiter,
}

impl AppState {
    /// Create new application state with database storage
    pub async fn new(storage: Storage) -> Self {
        let metrics = Arc::new(MetricsCollector::new());
        let tracker = Arc::new(TransactionTracker::new(metrics.clone()));
        let coordinator = Arc::new(tokio::sync::Mutex::new(CrossChainCoordinator::new()));

        // Rate limiter: configurable via env vars
        let rate_limit: u32 = std::env::var("RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);
        let rate_limiter = RateLimiter::new(rate_limit, 60);

        // Initialize Solana executor
        #[cfg(feature = "solana")]
        let solana_executor = std::env::var("SOLANA_RPC_URL").ok().map(|url| {
            info!("Configuring Solana executor: {}", url);
            let executor = SolanaExecutor::new(url, CommitmentConfig::confirmed());

            let executor = if let Ok(key) = std::env::var("SOLANA_PRIVATE_KEY") {
                match executor.with_keypair_base58(&key) {
                    Ok(e) => {
                        info!("Solana signing key loaded");
                        e
                    }
                    Err(e) => {
                        warn!("Failed to load Solana key: {}", e);
                        SolanaExecutor::new(
                            std::env::var("SOLANA_RPC_URL").unwrap(),
                            CommitmentConfig::confirmed(),
                        )
                    }
                }
            } else {
                executor
            };

            Arc::new(executor)
        });

        // Initialize Base executor
        let base_executor = std::env::var("BASE_RPC_URL").ok().map(|url| {
            info!("Configuring Base executor: {}", url);
            let executor = BaseExecutor::new(url.clone());

            let executor = if let Ok(key) = std::env::var("BASE_PRIVATE_KEY") {
                match executor.with_private_key(&key) {
                    Ok(e) => {
                        info!("Base signing key loaded");
                        e
                    }
                    Err(e) => {
                        warn!("Failed to load Base key: {}", e);
                        BaseExecutor::new(url)
                    }
                }
            } else {
                executor
            };

            Arc::new(executor)
        });

        // Initialize Shape executor (OP Stack L2, chain ID 360)
        let shape_executor = std::env::var("SHAPE_RPC_URL").ok().map(|url| {
            info!("Configuring Shape executor: {}", url);
            let executor = BaseExecutor::new(url.clone()).with_chain_id(360);

            let executor = if let Ok(key) = std::env::var("SHAPE_PRIVATE_KEY") {
                match executor.with_private_key(&key) {
                    Ok(e) => {
                        info!("Shape signing key loaded");
                        e
                    }
                    Err(e) => {
                        warn!("Failed to load Shape key: {}", e);
                        BaseExecutor::new(url).with_chain_id(360)
                    }
                }
            } else {
                executor
            };

            Arc::new(executor)
        });

        Self {
            metrics,
            tracker,
            coordinator,
            #[cfg(feature = "solana")]
            solana_executor,
            base_executor,
            shape_executor,
            storage: Arc::new(storage),
            rate_limiter,
        }
    }

    /// Get the executor for a given chain
    pub fn get_executor(&self, chain: Chain) -> Option<Arc<dyn Executor>> {
        match chain {
            Chain::Solana => {
                #[cfg(feature = "solana")]
                return self
                    .solana_executor
                    .clone()
                    .map(|e| e as Arc<dyn Executor>);
                #[cfg(not(feature = "solana"))]
                None
            }
            Chain::Base => self
                .base_executor
                .clone()
                .map(|e| e as Arc<dyn Executor>),
            Chain::Shape => self
                .shape_executor
                .clone()
                .map(|e| e as Arc<dyn Executor>),
        }
    }
}
