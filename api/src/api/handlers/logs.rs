use crate::db::DbClient;
use crate::services::logging::read_logs;
use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};

// Route handler for GET /logs/:id which checks the status of a job
pub(crate) async fn get_build_logs(
    State(db): State<DbClient>,
    Path(address): Path<String>,
) -> Json<Value> {
    let file_id = match db.get_logs_info(&address).await {
        Ok(res) => res.file_name,
        Err(err) => {
            tracing::error!("Error getting data from database: {}", err);
            return Json(json!({
                "error": "We could not find the logs for this program"
            }));
        }
    };

    let logs = read_logs(&file_id);

    Json(logs)
}
