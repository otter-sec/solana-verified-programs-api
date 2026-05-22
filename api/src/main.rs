use std::net::SocketAddr;

mod api;
mod config;
mod db;
mod errors;
mod responses;
mod services;
mod state;
mod validation;

use crate::state::AppState;

/// Result type for API
pub type Result<T> = std::result::Result<T, errors::ApiError>;

#[tokio::main]
async fn main() {
    // Initialize logging. `fmt::init()` alone filters everything when
    // RUST_LOG is unset; default to INFO so deployments aren't silent.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = config::Config::from_env().expect("Failed to load configuration");

    let db = db::DbClient::connect(&config.database_url, config.db_max_connections)
        .await
        .expect("Failed to connect to database");
    db.migrate().await.expect("Failed to apply migrations");

    match db.fail_orphan_builds().await {
        Ok(0) => {}
        Ok(n) => tracing::info!("Marked {} orphan in-progress builds as failed", n),
        Err(e) => tracing::error!("Failed to clean up orphan builds: {}", e),
    }

    let state = AppState::new(
        db.clone(),
        &config.rpc_url,
        &config.auth_secret,
        config.sweep_interval_seconds,
    );

    let bg_job_manager =
        services::background_jobs::BackgroundJobManager::new(&db, config.sweep_interval_seconds);
    let initial_health = bg_job_manager.get_health_status().await;
    tracing::info!("Background job initial status: {:?}", initial_health);

    services::background_jobs::spawn(db, state.rpc.clone(), config.sweep_interval_seconds);

    let app = api::initialize_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
