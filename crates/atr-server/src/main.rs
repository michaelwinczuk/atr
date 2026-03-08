//! Agent Transaction Router HTTP server

use atr_observer::Storage;
use atr_server::{routes, state::AppState};
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

    // Build router with auth middleware
    let app = routes::build_router(state)
        .layer(CorsLayer::permissive());

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
