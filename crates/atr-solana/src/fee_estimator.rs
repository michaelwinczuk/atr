//! Priority fee estimation for Solana

use atr_core::error::{AtrError, AtrResult};
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, warn};

/// Priority fee estimator with multi-RPC failover
pub struct PriorityFeeEstimator {
    rpc_urls: Vec<String>,
    client: Client,
}

impl PriorityFeeEstimator {
    /// Create a new fee estimator.
    /// `rpc_url` can be comma-separated for failover.
    pub fn new(rpc_url: String) -> Self {
        let rpc_urls: Vec<String> = rpc_url
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            rpc_urls,
            client: Client::new(),
        }
    }

    /// Make a JSON-RPC call with automatic failover
    async fn rpc_call(&self, method: &str, params: Value) -> AtrResult<Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let mut last_error = AtrError::RpcError("No RPC URLs configured".to_string());

        for url in &self.rpc_urls {
            match self.try_rpc_call(url, &body).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("RPC {} failed for {}: {}, trying next", url, method, e);
                    last_error = e;
                }
            }
        }

        Err(last_error)
    }

    async fn try_rpc_call(&self, url: &str, body: &Value) -> AtrResult<Value> {
        let response = self
            .client
            .post(url)
            .json(body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AtrError::RpcError(format!("HTTP error ({}): {}", url, e)))?;

        let result: Value = response
            .json()
            .await
            .map_err(|e| AtrError::RpcError(format!("JSON parse error: {}", e)))?;

        if let Some(error) = result.get("error") {
            return Err(AtrError::RpcError(format!("RPC error: {}", error)));
        }

        result
            .get("result")
            .cloned()
            .ok_or_else(|| AtrError::RpcError("Missing result field".to_string()))
    }

    /// Estimate priority fee based on recent blocks using getRecentPrioritizationFees
    pub async fn estimate_priority_fee(&self) -> AtrResult<u64> {
        debug!("Estimating Solana priority fee via getRecentPrioritizationFees");

        let result = self
            .rpc_call("getRecentPrioritizationFees", json!([]))
            .await;

        match result {
            Ok(fees) => {
                if let Some(fee_array) = fees.as_array() {
                    if fee_array.is_empty() {
                        debug!("No recent priority fees, using default");
                        return Ok(5000);
                    }

                    // Collect all non-zero priority fees
                    let mut priority_fees: Vec<u64> = fee_array
                        .iter()
                        .filter_map(|entry| {
                            entry
                                .get("prioritizationFee")
                                .and_then(|f| f.as_u64())
                        })
                        .filter(|&f| f > 0)
                        .collect();

                    if priority_fees.is_empty() {
                        debug!("All recent priority fees are zero, using minimum");
                        return Ok(1000);
                    }

                    // Sort and take the 75th percentile for reliable inclusion
                    priority_fees.sort_unstable();
                    let p75_index = (priority_fees.len() as f64 * 0.75) as usize;
                    let p75_index = p75_index.min(priority_fees.len() - 1);
                    let recommended_fee = priority_fees[p75_index];

                    // Cap at a reasonable maximum (10M microlamports)
                    let capped = recommended_fee.min(10_000_000);
                    debug!(
                        "Priority fee estimate: {} microlamports (p75 of {} samples)",
                        capped,
                        priority_fees.len()
                    );
                    Ok(capped)
                } else {
                    warn!("Unexpected response format for getRecentPrioritizationFees");
                    Ok(5000)
                }
            }
            Err(e) => {
                warn!("getRecentPrioritizationFees failed: {}, using default", e);
                Ok(5000) // Safe default: 5000 microlamports
            }
        }
    }

    /// Get current network congestion level (0.0 = empty, 1.0 = full)
    pub async fn get_congestion_level(&self) -> AtrResult<f64> {
        debug!("Checking Solana network congestion");

        // Query recent performance samples to estimate congestion
        let result = self
            .rpc_call("getRecentPerformanceSamples", json!([4]))
            .await;

        match result {
            Ok(samples) => {
                if let Some(sample_array) = samples.as_array() {
                    if sample_array.is_empty() {
                        return Ok(0.5);
                    }

                    // Calculate average transactions per slot vs theoretical max
                    let total_txs: u64 = sample_array
                        .iter()
                        .filter_map(|s| s.get("numTransactions").and_then(|n| n.as_u64()))
                        .sum();
                    let total_slots: u64 = sample_array
                        .iter()
                        .filter_map(|s| s.get("numSlots").and_then(|n| n.as_u64()))
                        .sum();

                    if total_slots == 0 {
                        return Ok(0.5);
                    }

                    let avg_txs_per_slot = total_txs as f64 / total_slots as f64;
                    // Solana theoretical max ~4000 tx/slot, practical ~2000
                    let congestion = (avg_txs_per_slot / 2000.0).min(1.0);
                    debug!("Congestion level: {:.2} ({:.0} tx/slot)", congestion, avg_txs_per_slot);
                    Ok(congestion)
                } else {
                    Ok(0.5)
                }
            }
            Err(e) => {
                warn!("getRecentPerformanceSamples failed: {}", e);
                Ok(0.5)
            }
        }
    }
}
