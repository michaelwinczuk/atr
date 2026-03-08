//! High-level SDK for Agent Transaction Router

use atr_core::{
    intent::TransactionIntent,
    transaction::TransactionRecord,
    error::{AtrError, AtrResult},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// ATR SDK client
pub struct AtrClient {
    base_url: String,
    client: Client,
}

impl AtrClient {
    /// Create a new ATR client
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    /// Submit a transaction intent
    pub async fn submit_intent(&self, intent: TransactionIntent) -> AtrResult<Uuid> {
        let url = format!("{}/api/v1/intents", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&intent)
            .send()
            .await
            .map_err(|e| AtrError::RpcError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(AtrError::SubmissionFailed(
                response.text().await.unwrap_or_default(),
            ));
        }

        let result: SubmitResponse = response
            .json()
            .await
            .map_err(|e| AtrError::SerializationError(e.to_string()))?;

        Ok(result.id)
    }

    /// Get intent status
    pub async fn get_status(&self, id: Uuid) -> AtrResult<TransactionRecord> {
        let url = format!("{}/api/v1/intents/{}", self.base_url, id);
        let response = self
            .client
            .get(&url)
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
    pub async fn cancel_intent(&self, id: Uuid) -> AtrResult<bool> {
        let url = format!("{}/api/v1/intents/{}/cancel", self.base_url, id);
        let response = self
            .client
            .post(&url)
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

        Ok(result.cancelled)
    }
}

#[derive(Deserialize)]
struct SubmitResponse {
    id: Uuid,
}

#[derive(Deserialize)]
struct CancelResponse {
    cancelled: bool,
}
