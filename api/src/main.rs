use anyhow::Context;
use axum::Server;
use config::Config;
use logging::setup_logging;
use std::net::SocketAddr;

mod api;
mod config;
mod db;
mod errors;
mod logging;
mod schema;
mod services;

/// Result type for API
pub type Result<T> = std::result::Result<T, errors::ApiError>;

/// Static configuration instance for the API
static CONFIG: once_cell::sync::Lazy<Config> = once_cell::sync::Lazy::new(|| {
    dotenv::dotenv().ok();
    envy::from_env::<Config>().expect("Failed to load configuration")
});

#[tokio::main]
async fn main() {
    // Initialize logging with persistent file-based logs
    _ = setup_logging().context("Failed to initialize logging");

    // Initialize database and Redis connections
    let db_client = db::DbClient::new(&CONFIG.database_url, &CONFIG.redis_url);

    // Setup API router and start server
    let app = api::initialize_router(db_client);
    let addr = SocketAddr::from(([0, 0, 0, 0], CONFIG.port));
    tracing::info!("Server starting on {}", addr);

    Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
