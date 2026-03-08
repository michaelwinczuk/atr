//! High-level SDK for Agent Transaction Router

use atr_core::{
    error::{AtrError, AtrResult},
    intent::TransactionIntent,
    transaction::TransactionRecord,
};
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

/// ATR SDK client
pub struct AtrClient {
    base_url: String,
    api_key: String,
    client: Client,
}

impl AtrClient {
    /// Create a new ATR client with API key authentication
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url,
            api_key,
            client: Client::new(),
        }
    }

    /// Submit a transaction intent
    pub async fn submit_intent(&self, intent: TransactionIntent) -> AtrResult<SubmitResult> {
        let url = format!("{}/api/v1/intents", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .json(&intent)
            .send()
            .await
            .map_err(|e| AtrError::RpcError(e.to_string()))?;

        if response.status().as_u16() == 401 {
            return Err(AtrError::ConfigError("Invalid API key".to_string()));
        }
        if response.status().as_u16() == 429 {
            return Err(AtrError::RpcError("Rate limit exceeded".to_string()));
        }
        if !response.status().is_success() {
            return Err(AtrError::SubmissionFailed(
                response.text().await.unwrap_or_default(),
            ));
        }

        let result: SubmitResponse = response
            .json()
            .await
            .map_err(|e| AtrError::SerializationError(e.to_string()))?;

        Ok(SubmitResult {
            id: result.id,
            status: result.status,
            tx_hash: result.tx_hash,
            estimated_fee: result.estimated_fee,
            error: result.error,
        })
    }

    /// Get intent status
    pub async fn get_status(&self, id: Uuid) -> AtrResult<TransactionRecord> {
        let url = format!("{}/api/v1/intents/{}", self.base_url, id);
        let response = self
            .client
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| AtrError::RpcError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(AtrError::Internal(
                response.text().await.unwrap_or_default(),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| AtrError::SerializationError(e.to_string()))
    }

    /// Cancel a pending intent
    pub async fn cancel_intent(&self, id: Uuid) -> AtrResult<CancelResult> {
        let url = format!("{}/api/v1/intents/{}/cancel", self.base_url, id);
        let response = self
            .client
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| AtrError::RpcError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(AtrError::Internal(
                response.text().await.unwrap_or_default(),
            ));
        }

        let result: CancelResponse = response
            .json()
            .await
            .map_err(|e| AtrError::SerializationError(e.to_string()))?;

        Ok(CancelResult {
            cancelled: result.cancelled,
            reason: result.reason,
        })
    }

    /// Poll for transaction completion (blocks until terminal state or timeout)
    pub async fn wait_for_confirmation(
        &self,
        id: Uuid,
        timeout_secs: u64,
    ) -> AtrResult<TransactionRecord> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);

        loop {
            let record = self.get_status(id).await?;
            if record.is_terminal() {
                return Ok(record);
            }

            if start.elapsed() >= timeout {
                return Err(AtrError::ConfirmationTimeout);
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
}

/// Result from submitting an intent
pub struct SubmitResult {
    pub id: Uuid,
    pub status: String,
    pub tx_hash: Option<String>,
    pub estimated_fee: Option<u64>,
    pub error: Option<String>,
}

/// Result from cancelling an intent
pub struct CancelResult {
    pub cancelled: bool,
    pub reason: String,
}

#[derive(Deserialize)]
struct SubmitResponse {
    id: Uuid,
    status: String,
    tx_hash: Option<String>,
    estimated_fee: Option<u64>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct CancelResponse {
    cancelled: bool,
    reason: String,
}
