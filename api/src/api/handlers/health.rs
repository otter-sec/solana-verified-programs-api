use crate::{db::DbClient, services::background_jobs::BackgroundJobManager};
use axum::{extract::State, http::StatusCode, Json};

/// Health check endpoint that includes background job status
pub async fn health_check(State(db): State<DbClient>) -> (StatusCode, Json<serde_json::Value>) {
    let bg_manager = BackgroundJobManager::new(db.clone());
    let bg_health = bg_manager.get_health_status().await;

    let redis_status = match db.get_async_redis_conn().await {
        Err(e) => serde_json::json!({
            "status": "error",
            "message": e.to_string()
        }),
        Ok(_) => serde_json::json!("connected"),
    };

    let health_status = serde_json::json!({
        "status": "ok",
        "database": "connected",
        "redis": redis_status,
        "background_jobs": bg_health,
        "timestamp": chrono::Utc::now()
    });

    (StatusCode::OK, Json(health_status))
}

/// Background job status endpoint
pub async fn background_job_status(
    State(db): State<DbClient>,
) -> (
    StatusCode,
    Json<crate::services::background_jobs::BackgroundJobHealth>,
) {
    let bg_manager = BackgroundJobManager::new(db);
    let health = bg_manager.get_health_status().await;

    let status_code = match health.status.as_str() {
        "healthy" => StatusCode::OK,
        "unknown" => StatusCode::ACCEPTED,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };

    (status_code, Json(health))
}
