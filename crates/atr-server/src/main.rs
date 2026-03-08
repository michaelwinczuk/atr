//! Agent Transaction Router HTTP server

use atr_observer::Storage;
use atr_server::{poller, routes, state::AppState};
use tower_http::cors::CorsLayer;
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize SQLite storage
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:atr.db?mode=rwc".to_string());
    let storage = Storage::new(&db_url)
        .await
        .expect("Failed to initialize database");

    // Create application state
    let state = AppState::new(storage).await;

    // Bootstrap: create a default API key if none exist
    if let Ok(keys) = state.storage.list_api_keys().await {
        if keys.is_empty() {
            match state.storage.create_api_key("default").await {
                Ok(key) => {
                    info!("===========================================");
                    info!("  No API keys found. Created default key:");
                    info!("  {}", key);
                    info!("  Use: X-API-Key: {}", key);
                    info!("===========================================");
                }
                Err(e) => tracing::error!("Failed to create default API key: {}", e),
            }
        }
    }

    // Start background confirmation poller
    let poll_interval: u64 = std::env::var("POLL_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let poller_state = state.clone();
    tokio::spawn(async move {
        poller::run_confirmation_poller(poller_state, poll_interval).await;
    });

    // Start rate limiter cleanup task (every 5 minutes)
    let cleanup_limiter = state.rate_limiter.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            cleanup_limiter.cleanup();
        }
    });

    // Build router
    let app = routes::build_router(state).layer(CorsLayer::permissive());

    // Start server
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting ATR server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
