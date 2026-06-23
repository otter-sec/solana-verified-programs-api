use crate::{build::logs::read_logs, db::DbClient};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use tracing::info;
use uuid::Uuid;

/// Handler for retrieving build logs for a specific program
///
/// # Endpoint: GET /logs/{build_id}
pub(crate) async fn get_build_logs(
    State(db): State<DbClient>,
    Path(build_id): Path<String>,
) -> (StatusCode, Json<Value>) {
    let Ok(id) = Uuid::parse_str(&build_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid build id (expected UUID)" })),
        );
    };

    info!("Fetching build logs for build_id: {}", build_id);

    let Ok(Some(file_name)) = db.get_build_log_file(id).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "We could not find the logs for this build" })),
        );
    };

    (StatusCode::OK, Json(read_logs(&file_name).await))
}
