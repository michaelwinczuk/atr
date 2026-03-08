//! Background confirmation poller
//!
//! Periodically checks the status of submitted transactions and updates
//! the tracker and storage when confirmations arrive.

use crate::state::AppState;
use atr_core::transaction::TransactionStatus;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Run the confirmation poller as a background task.
/// Polls every `interval_secs` seconds for non-terminal transactions
/// and updates their status via the chain executors.
pub async fn run_confirmation_poller(state: AppState, interval_secs: u64) {
    let interval = Duration::from_secs(interval_secs);
    info!(
        "Starting confirmation poller (interval: {}s)",
        interval_secs
    );

    loop {
        tokio::time::sleep(interval).await;

        let pending = match state.storage.get_pending_transactions().await {
            Ok(txs) => txs,
            Err(e) => {
                error!("Poller: failed to fetch pending transactions: {}", e);
                continue;
            }
        };

        if pending.is_empty() {
            continue;
        }

        debug!("Poller: checking {} pending transactions", pending.len());

        for record in &pending {
            // Only poll transactions that have been submitted (have a tx_hash)
            let tx_hash = match &record.tx_hash {
                Some(hash) => hash.clone(),
                None => continue,
            };

            // Skip if already in a non-pollable state
            if !matches!(
                record.status,
                TransactionStatus::Submitted | TransactionStatus::Confirmed
            ) {
                continue;
            }

            let executor = match state.get_executor(record.chain) {
                Some(e) => e,
                None => continue,
            };

            match executor.check_status(&tx_hash).await {
                Ok(updated) => {
                    if updated.status != record.status {
                        info!(
                            "Poller: tx {} status {:?} -> {:?}",
                            tx_hash, record.status, updated.status
                        );
                        state.tracker.update_status(record.id, updated.status);
                        let _ = state
                            .storage
                            .update_transaction_status(record.id, updated.status)
                            .await;

                        // Update additional fields if available
                        if let Some(block) = updated.block_number {
                            let _ = state
                                .storage
                                .update_transaction_block(record.id, block)
                                .await;
                        }
                        if let Some(fee) = updated.fee_paid {
                            let _ = state
                                .storage
                                .update_transaction_fee(record.id, fee)
                                .await;
                        }
                    }
                }
                Err(e) => {
                    warn!("Poller: failed to check tx {}: {}", tx_hash, e);
                }
            }
        }
    }
}
