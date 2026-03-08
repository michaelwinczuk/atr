//! API route definitions

use axum::{
    routing::{get, post},
    Router,
};

use crate::handlers;
use crate::state::AppState;

/// Build API routes
pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/intents", post(handlers::submit_intent))
        .route("/intents/:id", get(handlers::get_intent_status))
        .route("/intents/:id/cancel", post(handlers::cancel_intent))
        .route("/metrics", get(handlers::metrics))
}
