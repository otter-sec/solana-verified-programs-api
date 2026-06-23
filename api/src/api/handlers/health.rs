use crate::{responses::BackgroundJobStatus, services::background_jobs, state::AppState};
use axum::{extract::State, http::StatusCode, Json};

pub async fn health_check(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let bg_health = background_jobs::health(&state);
    let bg_ok = bg_health.status == BackgroundJobStatus::Active;

    let (db_status, db_ok) = match state.db.ping().await {
        Ok(_) => (serde_json::json!("connected"), true),
        Err(e) => (
            serde_json::json!({
                "status": "error",
                "message": e.to_string()
            }),
            false,
        ),
    };

    let overall_ok = bg_ok && db_ok;

    let health_status = serde_json::json!({
        "status": if overall_ok { "ok" } else { "degraded" },
        "database": db_status,
        "background_jobs": bg_health,
        "timestamp": chrono::Utc::now()
    });

    let status_code = if overall_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status_code, Json(health_status))
}

pub async fn background_job_status(
    State(state): State<AppState>,
) -> (StatusCode, Json<crate::responses::BackgroundJobHealth>) {
    let health = background_jobs::health(&state);

    let status_code = match health.status {
        BackgroundJobStatus::Active => StatusCode::OK,
        BackgroundJobStatus::Unknown => StatusCode::ACCEPTED,
        BackgroundJobStatus::Inactive => StatusCode::SERVICE_UNAVAILABLE,
    };

    (status_code, Json(health))
}
