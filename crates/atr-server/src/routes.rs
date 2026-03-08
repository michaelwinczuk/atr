//! API route definitions

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::auth::auth_middleware;
use crate::handlers;
use crate::state::AppState;

/// Build the full application router
pub fn build_router(state: AppState) -> Router {
    // Public routes — no auth required
    let public = Router::new()
        .route("/api/v1/health", get(handlers::health))
        .route("/api/v1/metrics", get(handlers::metrics));

    // Protected routes — require valid X-API-Key header
    let protected = Router::new()
        .route("/api/v1/intents", post(handlers::submit_intent))
        .route("/api/v1/intents/:id", get(handlers::get_intent_status))
        .route(
            "/api/v1/intents/:id/cancel",
            post(handlers::cancel_intent),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Admin routes — require X-Admin-Key matching ATR_ADMIN_KEY env var
    let admin = Router::new()
        .route("/admin/keys", post(handlers::create_api_key))
        .route("/admin/keys", get(handlers::list_api_keys));

    public
        .merge(protected)
        .merge(admin)
        .with_state(state)
}
