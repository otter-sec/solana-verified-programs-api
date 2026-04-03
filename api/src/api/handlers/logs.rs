use crate::{db::DbClient, services::logging::read_logs};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use tracing::{error, info};
use uuid::Uuid;

/// Handler for retrieving build logs for a specific program
///
/// # Endpoint: GET /logs/:build_id
///
/// # Arguments
/// * `db` - Database client from application state
/// * `build_id` - Build id to fetch logs
///
/// # Returns
/// * `(StatusCode, Json<Value>)` - HTTP status and JSON response containing either the logs or an error message
pub(crate) async fn get_build_logs(
    State(db): State<DbClient>,
    Path(build_id): Path<String>,
) -> (StatusCode, Json<Value>) {
    if Uuid::parse_str(&build_id).is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid build id (expected UUID)" })),
        );
    }

    info!("Fetching build logs for build_id: {}", build_id);

    let file_id = match db.get_logs_info(&build_id).await {
        Ok(res) => {
            info!("Found log file: {}", res.file_name);
            res.file_name
        }
        Err(err) => {
            error!("Failed to retrieve logs from database: {}", err);
            return (
                StatusCode::OK,
                Json(json!({
                    "error": "We could not find the logs for this build"
                })),
            );
        }
    };

    let logs = read_logs(&file_id).await;
    info!("Successfully retrieved logs for build_id: {}", build_id);

    (StatusCode::OK, Json(logs))
}
