//! HTTP request handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::{info, warn};
use uuid::Uuid;

use atr_core::{
    intent::TransactionIntent,
    transaction::{TransactionRecord, TransactionStatus},
};

use crate::state::AppState;

/// Health check endpoint
pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Submit a transaction intent — simulate and execute
pub async fn submit_intent(
    State(state): State<AppState>,
    Json(intent): Json<TransactionIntent>,
) -> Result<Json<SubmitResponse>, AppError> {
    info!("Received intent {} for chain {}", intent.id, intent.chain);

    // Check idempotency
    if let Some(key) = &intent.idempotency_key {
        if let Ok(Some(existing_id)) = state.storage.check_idempotency(key, intent.id).await {
            if existing_id != intent.id {
                return Ok(Json(SubmitResponse {
                    id: existing_id,
                    status: "duplicate".to_string(),
                    tx_hash: None,
                    estimated_fee: None,
                    error: Some("Idempotency key already used".to_string()),
                }));
            }
        }
    }

    // Track the intent
    let record = TransactionRecord::new(intent.id, intent.chain);
    state.tracker.track(record.clone());
    let _ = state.storage.save_transaction(&record).await;

    // Get executor for chain
    let executor = state.get_executor(intent.chain);
    match executor {
        Some(exec) => {
            // Simulate
            state
                .tracker
                .update_status(intent.id, TransactionStatus::Simulating);
            let _ = state
                .storage
                .update_transaction_status(intent.id, TransactionStatus::Simulating)
                .await;

            match exec.simulate(&intent).await {
                Ok(sim) => {
                    if !sim.success {
                        state
                            .tracker
                            .update_status(intent.id, TransactionStatus::SimulationFailed);
                        let _ = state
                            .storage
                            .update_transaction_status(
                                intent.id,
                                TransactionStatus::SimulationFailed,
                            )
                            .await;

                        return Ok(Json(SubmitResponse {
                            id: intent.id,
                            status: "simulation_failed".to_string(),
                            tx_hash: None,
                            estimated_fee: None,
                            error: sim.error,
                        }));
                    }

                    state
                        .tracker
                        .update_status(intent.id, TransactionStatus::Simulated);
                    info!("Simulation passed for intent {}", intent.id);

                    // Submit
                    match exec.submit(&intent).await {
                        Ok(tx_hash) => {
                            state
                                .tracker
                                .update_status(intent.id, TransactionStatus::Submitted);
                            let _ = state
                                .storage
                                .update_transaction_status(
                                    intent.id,
                                    TransactionStatus::Submitted,
                                )
                                .await;
                            let _ = state.storage.set_tx_hash(intent.id, &tx_hash).await;
                            info!("Intent {} submitted as tx {}", intent.id, tx_hash);

                            Ok(Json(SubmitResponse {
                                id: intent.id,
                                status: "submitted".to_string(),
                                tx_hash: Some(tx_hash),
                                estimated_fee: sim.estimated_fee,
                                error: None,
                            }))
                        }
                        Err(e) => {
                            warn!("Submission failed for intent {}: {}", intent.id, e);
                            state
                                .tracker
                                .update_status(intent.id, TransactionStatus::Failed);
                            let _ = state
                                .storage
                                .update_transaction_status(intent.id, TransactionStatus::Failed)
                                .await;

                            Ok(Json(SubmitResponse {
                                id: intent.id,
                                status: "submission_failed".to_string(),
                                tx_hash: None,
                                estimated_fee: sim.estimated_fee,
                                error: Some(e.to_string()),
                            }))
                        }
                    }
                }
                Err(e) => {
                    warn!("Simulation error for intent {}: {}", intent.id, e);
                    state
                        .tracker
                        .update_status(intent.id, TransactionStatus::SimulationFailed);
                    let _ = state
                        .storage
                        .update_transaction_status(intent.id, TransactionStatus::SimulationFailed)
                        .await;

                    Ok(Json(SubmitResponse {
                        id: intent.id,
                        status: "simulation_error".to_string(),
                        tx_hash: None,
                        estimated_fee: None,
                        error: Some(e.to_string()),
                    }))
                }
            }
        }
        None => Ok(Json(SubmitResponse {
            id: intent.id,
            status: "unsupported_chain".to_string(),
            tx_hash: None,
            estimated_fee: None,
            error: Some(format!(
                "No executor configured for chain {}",
                intent.chain
            )),
        })),
    }
}

/// Get intent status
pub async fn get_intent_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TransactionRecord>, AppError> {
    // Try in-memory first, fall back to DB
    if let Some(record) = state.tracker.get(id) {
        return Ok(Json(record));
    }

    match state.storage.get_transaction(id).await {
        Ok(Some(record)) => Ok(Json(record)),
        _ => Err(AppError::NotFound),
    }
}

/// Cancel a pending intent
pub async fn cancel_intent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CancelResponse>, AppError> {
    let record = state.tracker.get(id).ok_or(AppError::NotFound)?;

    if record.is_terminal() {
        return Ok(Json(CancelResponse {
            id,
            cancelled: false,
            reason: format!(
                "Transaction already in terminal state: {:?}",
                record.status
            ),
        }));
    }

    let executor = state.get_executor(record.chain);
    match executor {
        Some(exec) => {
            if let Some(tx_hash) = &record.tx_hash {
                match exec.cancel(tx_hash).await {
                    Ok(new_hash) => {
                        state
                            .tracker
                            .update_status(id, TransactionStatus::Dropped);
                        let _ = state
                            .storage
                            .update_transaction_status(id, TransactionStatus::Dropped)
                            .await;
                        Ok(Json(CancelResponse {
                            id,
                            cancelled: true,
                            reason: format!("Replacement tx: {}", new_hash),
                        }))
                    }
                    Err(e) => Ok(Json(CancelResponse {
                        id,
                        cancelled: false,
                        reason: e.to_string(),
                    })),
                }
            } else {
                state
                    .tracker
                    .update_status(id, TransactionStatus::Dropped);
                let _ = state
                    .storage
                    .update_transaction_status(id, TransactionStatus::Dropped)
                    .await;
                Ok(Json(CancelResponse {
                    id,
                    cancelled: true,
                    reason: "Intent dropped before submission".to_string(),
                }))
            }
        }
        None => Ok(Json(CancelResponse {
            id,
            cancelled: false,
            reason: "No executor configured for chain".to_string(),
        })),
    }
}

/// Metrics endpoint
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let snapshot = state.metrics.snapshot();
    Json(serde_json::json!({
        "transactions_submitted": snapshot.submissions,
        "transactions_confirmed": snapshot.confirmations,
        "transactions_failed": snapshot.failures,
        "avg_confirmation_time_secs": snapshot.avg_confirmation_time,
        "active_intents": state.tracker.get_all().len(),
    }))
}

/// Create a new API key (admin endpoint)
pub async fn create_api_key(
    State(state): State<AppState>,
    Json(body): Json<CreateKeyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Validate admin key from header would go here
    match state.storage.create_api_key(&body.name).await {
        Ok(key) => Ok(Json(serde_json::json!({
            "key": key,
            "name": body.name,
            "message": "Store this key securely — it cannot be retrieved later"
        }))),
        Err(e) => Err(AppError::Internal(e.to_string())),
    }
}

/// List API keys (admin endpoint)
pub async fn list_api_keys(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    match state.storage.list_api_keys().await {
        Ok(keys) => {
            let keys_json: Vec<serde_json::Value> = keys
                .into_iter()
                .map(|(key, name, active)| {
                    // Mask the key for security
                    let masked = format!("{}...{}", &key[..8], &key[key.len() - 4..]);
                    serde_json::json!({
                        "key": masked,
                        "name": name,
                        "active": active,
                    })
                })
                .collect();
            Ok(Json(serde_json::json!({ "keys": keys_json })))
        }
        Err(e) => Err(AppError::Internal(e.to_string())),
    }
}

#[derive(serde::Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct SubmitResponse {
    id: Uuid,
    status: String,
    tx_hash: Option<String>,
    estimated_fee: Option<u64>,
    error: Option<String>,
}

#[derive(Serialize)]
pub struct CancelResponse {
    id: Uuid,
    cancelled: bool,
    reason: String,
}

/// Application error type
pub enum AppError {
    NotFound,
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Intent not found".to_string()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
