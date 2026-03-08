//! EIP-1559 gas estimation for Base

use atr_core::error::{AtrError, AtrResult};
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, warn};

/// Gas estimator for Base transactions with multi-RPC failover
pub struct GasEstimator {
    /// Primary RPC URL
    rpc_urls: Vec<String>,
    client: Client,
}

impl GasEstimator {
    /// Create a new gas estimator.
    /// The `rpc_url` can be a single URL or comma-separated list for failover.
    /// Example: `"https://mainnet.base.org,https://base.llamarpc.com"`
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

    /// Make a JSON-RPC call with automatic failover across configured RPCs
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

    /// Attempt a single RPC call to one URL
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

    /// Estimate gas limit for a simple transfer using eth_estimateGas
    pub async fn estimate_gas(&self) -> AtrResult<u64> {
        debug!("Estimating gas limit via eth_estimateGas");

        // For a simple ETH transfer, 21,000 is the exact gas cost.
        // For contract interactions, use estimate_gas_for_tx() which calls eth_estimateGas.
        Ok(21_000)
    }

    /// Estimate gas for a specific transaction call
    pub async fn estimate_gas_for_tx(
        &self,
        from: &str,
        to: &str,
        value: Option<u64>,
        data: Option<&str>,
    ) -> AtrResult<u64> {
        let mut tx_obj = json!({
            "from": from,
        });
        if !to.is_empty() {
            tx_obj["to"] = json!(to);
        }

        if let Some(v) = value {
            tx_obj["value"] = json!(format!("0x{:x}", v));
        }
        if let Some(d) = data {
            tx_obj["data"] = json!(d);
        }

        let result = self.rpc_call("eth_estimateGas", json!([tx_obj])).await?;

        let gas_hex = result
            .as_str()
            .ok_or_else(|| AtrError::RpcError("Invalid gas estimate response".to_string()))?;

        let gas = u64::from_str_radix(gas_hex.trim_start_matches("0x"), 16)
            .map_err(|e| AtrError::RpcError(format!("Invalid gas hex: {}", e)))?;

        // Add 20% safety buffer
        let buffered = gas + gas / 5;
        debug!("Estimated gas: {} (buffered: {})", gas, buffered);
        Ok(buffered)
    }

    /// Estimate base fee for next block using eth_feeHistory
    pub async fn estimate_base_fee(&self) -> AtrResult<u64> {
        debug!("Estimating base fee via eth_feeHistory");

        let result = self
            .rpc_call("eth_feeHistory", json!(["0x4", "latest", []]))
            .await;

        match result {
            Ok(fee_history) => {
                if let Some(base_fees) = fee_history.get("baseFeePerGas") {
                    if let Some(fees) = base_fees.as_array() {
                        // Use the latest base fee (last element is the pending block prediction)
                        if let Some(latest) = fees.last() {
                            if let Some(hex) = latest.as_str() {
                                let fee =
                                    u64::from_str_radix(hex.trim_start_matches("0x"), 16)
                                        .unwrap_or(1_000_000_000);
                                debug!("Base fee from eth_feeHistory: {} wei", fee);
                                return Ok(fee);
                            }
                        }
                    }
                }
                warn!("Could not parse eth_feeHistory, using default");
                Ok(1_000_000_000) // 1 gwei fallback
            }
            Err(e) => {
                warn!("eth_feeHistory failed: {}, using default", e);
                Ok(1_000_000_000) // 1 gwei fallback
            }
        }
    }

    /// Estimate priority fee (tip) using eth_maxPriorityFeePerGas
    pub async fn estimate_priority_fee(&self) -> AtrResult<u64> {
        debug!("Estimating priority fee via eth_maxPriorityFeePerGas");

        let result = self.rpc_call("eth_maxPriorityFeePerGas", json!([])).await;

        match result {
            Ok(value) => {
                if let Some(hex) = value.as_str() {
                    let fee = u64::from_str_radix(hex.trim_start_matches("0x"), 16)
                        .unwrap_or(1_000_000_000);
                    debug!("Priority fee: {} wei", fee);
                    Ok(fee)
                } else {
                    Ok(1_000_000_000) // 1 gwei fallback
                }
            }
            Err(e) => {
                warn!("eth_maxPriorityFeePerGas failed: {}, using default", e);
                Ok(1_000_000_000)
            }
        }
    }

    /// Estimate L1 data availability cost (Base-specific)
    /// Base uses an L1 gas price oracle at 0x420000000000000000000000000000000000000F
    pub async fn estimate_l1_cost(&self) -> AtrResult<u64> {
        debug!("Estimating L1 data cost via gas oracle");

        // Call the L1 gas oracle's l1BaseFee() method
        let oracle_address = "0x420000000000000000000000000000000000000F";
        // l1BaseFee() selector = 0x519b4bd3
        let result = self
            .rpc_call(
                "eth_call",
                json!([
                    {
                        "to": oracle_address,
                        "data": "0x519b4bd3"
                    },
                    "latest"
                ]),
            )
            .await;

        match result {
            Ok(value) => {
                if let Some(hex) = value.as_str() {
                    let l1_base_fee = u64::from_str_radix(
                        hex.trim_start_matches("0x").trim_start_matches('0'),
                        16,
                    )
                    .unwrap_or(10_000);
                    debug!("L1 base fee: {} wei", l1_base_fee);
                    // Approximate L1 cost: l1_base_fee * 16 * estimated_calldata_bytes
                    // For a typical transfer (~68 bytes of calldata)
                    Ok(l1_base_fee.saturating_mul(16).saturating_mul(68))
                } else {
                    Ok(10_000)
                }
            }
            Err(e) => {
                warn!("L1 gas oracle query failed: {}, using default", e);
                Ok(10_000)
            }
        }
    }

    /// Get current block number
    pub async fn get_block_number(&self) -> AtrResult<u64> {
        let result = self.rpc_call("eth_blockNumber", json!([])).await?;
        let hex = result
            .as_str()
            .ok_or_else(|| AtrError::RpcError("Invalid block number".to_string()))?;
        u64::from_str_radix(hex.trim_start_matches("0x"), 16)
            .map_err(|e| AtrError::RpcError(format!("Invalid block number hex: {}", e)))
    }

    /// Get transaction count (nonce) for an address
    pub async fn get_transaction_count(&self, address: &str) -> AtrResult<u64> {
        let result = self
            .rpc_call("eth_getTransactionCount", json!([address, "pending"]))
            .await?;
        let hex = result
            .as_str()
            .ok_or_else(|| AtrError::RpcError("Invalid nonce response".to_string()))?;
        u64::from_str_radix(hex.trim_start_matches("0x"), 16)
            .map_err(|e| AtrError::RpcError(format!("Invalid nonce hex: {}", e)))
    }

    /// Get transaction receipt
    pub async fn get_transaction_receipt(&self, tx_hash: &str) -> AtrResult<Option<Value>> {
        let result = self
            .rpc_call("eth_getTransactionReceipt", json!([tx_hash]))
            .await?;
        if result.is_null() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    /// Send raw transaction
    pub async fn send_raw_transaction(&self, raw_tx: &str) -> AtrResult<String> {
        let result = self
            .rpc_call("eth_sendRawTransaction", json!([raw_tx]))
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AtrError::RpcError("Invalid tx hash response".to_string()))
    }

    /// Simulate transaction via eth_call
    pub async fn eth_call(
        &self,
        from: &str,
        to: &str,
        value: Option<u64>,
        data: Option<&str>,
    ) -> AtrResult<String> {
        let mut tx_obj = json!({
            "from": from,
            "to": to,
        });
        if let Some(v) = value {
            tx_obj["value"] = json!(format!("0x{:x}", v));
        }
        if let Some(d) = data {
            tx_obj["data"] = json!(d);
        }

        let result = self
            .rpc_call("eth_call", json!([tx_obj, "latest"]))
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AtrError::RpcError("Invalid eth_call response".to_string()))
    }
}
