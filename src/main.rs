use std::net::SocketAddr;
use verified_programs_api::{api::routes, config, db, state::AppState, sweep};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = config::Config::from_env().expect("Failed to load configuration");

    let db = db::DbClient::connect(
        &config.database_url,
        config.db_max_connections,
        std::time::Duration::from_secs(config.sweep_interval_seconds),
    )
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
        config.max_reverifies_per_sweep,
    );

    sweep::spawn(state.clone());

    let app = routes::initialize_router(state);
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
