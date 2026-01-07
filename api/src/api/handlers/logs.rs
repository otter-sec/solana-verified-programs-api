use crate::{db::DbClient, services::logging::read_logs};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use tracing::{error, info};

/// Handler for retrieving build logs for a specific program
///
/// # Endpoint: GET /logs/:id
///
/// # Arguments
/// * `db` - Database client from application state
/// * `address` - Program address to fetch logs for
///
/// # Returns
/// * `(StatusCode, Json<Value>)` - HTTP status and JSON response containing either the logs or an error message
pub(crate) async fn get_build_logs(
    State(db): State<DbClient>,
    Path(address): Path<String>,
) -> (StatusCode, Json<Value>) {
    info!("Fetching build logs for program: {}", address);

    let file_id = match db.get_logs_info(&address).await {
        Ok(res) => {
            info!("Found log file: {}", res.file_name);
            res.file_name
        }
        Err(err) => {
            error!("Failed to retrieve logs from database: {}", err);
            return (
                StatusCode::OK,
                Json(json!({
                    "error": "We could not find the logs for this program"
                })),
            );
        }
    };

    let logs = read_logs(&file_id).await;
    info!("Successfully retrieved logs for program: {}", address);

    (StatusCode::OK, Json(logs))
}
