//! HTTP request handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

use atr_core::{
    chain::Chain,
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

    // Track the intent
    let mut record = TransactionRecord::new(intent.id, intent.chain);
    state.tracker.track(record.clone());

    // Simulate first
    let executor = state.get_executor(intent.chain);
    match executor {
        Some(exec) => {
            // Run simulation
            record.update_status(TransactionStatus::Simulating);
            state.tracker.update_status(intent.id, TransactionStatus::Simulating);

            match exec.simulate(&intent).await {
                Ok(sim) => {
                    if !sim.success {
                        state.tracker.update_status(intent.id, TransactionStatus::SimulationFailed);
                        return Ok(Json(SubmitResponse {
                            id: intent.id,
                            status: "simulation_failed".to_string(),
                            tx_hash: None,
                            estimated_fee: None,
                            error: sim.error,
                        }));
                    }

                    state.tracker.update_status(intent.id, TransactionStatus::Simulated);
                    info!("Simulation passed for intent {}", intent.id);

                    // Submit transaction
                    match exec.submit(&intent).await {
                        Ok(tx_hash) => {
                            state.tracker.update_status(intent.id, TransactionStatus::Submitted);
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
                            state.tracker.update_status(intent.id, TransactionStatus::Failed);

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
                    state.tracker.update_status(intent.id, TransactionStatus::SimulationFailed);

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
        None => {
            Ok(Json(SubmitResponse {
                id: intent.id,
                status: "unsupported_chain".to_string(),
                tx_hash: None,
                estimated_fee: None,
                error: Some(format!("No executor configured for chain {}", intent.chain)),
            }))
        }
    }
}

/// Get intent status
pub async fn get_intent_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TransactionRecord>, AppError> {
    state
        .tracker
        .get(id)
        .map(Json)
        .ok_or(AppError::NotFound)
}

/// Cancel a pending intent
pub async fn cancel_intent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CancelResponse>, AppError> {
    // Look up the intent's chain from the tracker
    let record = state.tracker.get(id).ok_or(AppError::NotFound)?;

    if record.is_terminal() {
        return Ok(Json(CancelResponse {
            id,
            cancelled: false,
            reason: format!("Transaction already in terminal state: {:?}", record.status),
        }));
    }

    let executor = state.get_executor(record.chain);
    match executor {
        Some(exec) => {
            if let Some(tx_hash) = &record.tx_hash {
                match exec.cancel(tx_hash).await {
                    Ok(new_hash) => {
                        state.tracker.update_status(id, TransactionStatus::Dropped);
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
                // No tx hash yet — just mark as dropped
                state.tracker.update_status(id, TransactionStatus::Dropped);
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

/// Metrics endpoint — returns key metrics as JSON
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
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Intent not found"),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
