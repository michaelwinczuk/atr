//! Authentication and rate limiting middleware

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

use crate::state::AppState;

/// Authentication + rate limiting middleware for protected routes.
/// Validates X-API-Key header against the database and enforces rate limits.
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let api_key = request
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok());

    match api_key {
        Some(key) => {
            // Validate key
            let valid = state
                .storage
                .validate_api_key(key)
                .await
                .unwrap_or(false);

            if !valid {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": "Invalid API key"})),
                )
                    .into_response());
            }

            // Check rate limit
            match state.rate_limiter.check(key) {
                Ok(remaining) => {
                    let mut response = next.run(request).await;
                    // Add rate limit headers
                    let headers = response.headers_mut();
                    headers.insert(
                        "X-RateLimit-Remaining",
                        remaining.to_string().parse().unwrap(),
                    );
                    Ok(response)
                }
                Err(retry_after) => Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({
                        "error": "Rate limit exceeded",
                        "retry_after_secs": retry_after,
                    })),
                )
                    .into_response()),
            }
        }
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Missing X-API-Key header"})),
        )
            .into_response()),
    }
}

/// Admin authentication middleware.
/// Validates X-Admin-Key header against the ATR_ADMIN_KEY environment variable.
pub async fn admin_auth_middleware(
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let admin_key = std::env::var("ATR_ADMIN_KEY").unwrap_or_default();

    if admin_key.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Admin API not configured. Set ATR_ADMIN_KEY env var."})),
        )
            .into_response());
    }

    let provided = request
        .headers()
        .get("x-admin-key")
        .and_then(|v| v.to_str().ok());

    match provided {
        Some(key) if key == admin_key => Ok(next.run(request).await),
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Invalid admin key"})),
        )
            .into_response()),
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Missing X-Admin-Key header"})),
        )
            .into_response()),
    }
}
