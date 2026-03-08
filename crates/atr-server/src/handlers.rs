//! HTTP request handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use atr_core::{
    intent::TransactionIntent,
    transaction::TransactionRecord,
};

use crate::state::AppState;

/// Health check endpoint
pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Submit a transaction intent
pub async fn submit_intent(
    State(state): State<AppState>,
    Json(intent): Json<TransactionIntent>,
) -> Result<Json<SubmitResponse>, AppError> {
    // Track the intent
    let record = TransactionRecord::new(intent.id, intent.chain);
    state.tracker.track(record);

    // TODO [Phase 2]: Actually execute the intent
    // - Simulate transaction
    // - Submit to appropriate executor
    // - Track status

    Ok(Json(SubmitResponse {
        id: intent.id,
        status: "pending".to_string(),
    }))
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
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CancelResponse>, AppError> {
    // TODO [Phase 2]: Implement cancellation
    Ok(Json(CancelResponse {
        id,
        cancelled: false,
        reason: "Cancellation not yet implemented".to_string(),
    }))
}

/// Metrics endpoint
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    // TODO [Phase 2]: Implement Prometheus metrics export
    Json(serde_json::json!({
        "message": "Metrics endpoint not yet implemented"
    }))
}

#[derive(Serialize)]
pub struct SubmitResponse {
    id: Uuid,
    status: String,
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
