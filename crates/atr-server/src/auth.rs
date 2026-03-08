//! API key authentication middleware

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

use crate::state::AppState;

/// Authentication middleware that validates API keys from the X-API-Key header.
/// Health and metrics endpoints bypass auth.
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    // Extract API key from header
    let api_key = request
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok());

    match api_key {
        Some(key) => {
            let valid = state
                .storage
                .validate_api_key(key)
                .await
                .unwrap_or(false);

            if valid {
                Ok(next.run(request).await)
            } else {
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": "Invalid API key"})),
                )
                    .into_response())
            }
        }
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Missing X-API-Key header"})),
        )
            .into_response()),
    }
}
