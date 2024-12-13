use axum::Server;
use config::Config;
use std::net::SocketAddr;

mod api;
mod config;
mod db;
mod errors;
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
    // Initialize logging
    tracing_subscriber::fmt::init();

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
