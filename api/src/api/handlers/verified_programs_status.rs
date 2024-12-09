use crate::db::{models::{Status, VerifiedProgramsStatusListResponse}, DbClient};
use axum::{extract::State, http::StatusCode, Json};
use tracing::{error, info};

/// Handler for retrieving status of all verified programs
///
/// # Endpoint: GET /verified-programs/status
///
/// # Returns
/// * `(StatusCode, Json<VerifiedProgramsStatusListResponse>)` - Status and list of program statuses
pub(crate) async fn get_verified_programs_status(
    State(db): State<DbClient>,
) -> (StatusCode, Json<VerifiedProgramsStatusListResponse>) {
    info!("Fetching status for all verified programs");

    let all_verified_programs = db.get_verification_status_all().await;

    match all_verified_programs {
        Ok(all_verified_programs) => {
            info!("Successfully retrieved status for all programs");
    (
        StatusCode::OK,
        Json(VerifiedProgramsStatusListResponse {
            status: Status::Success,
            data: Some(all_verified_programs),
                error: None,
            }),
        )
    }
    Err(err) => {
            error!("Failed to fetch verified programs from database: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(VerifiedProgramsStatusListResponse {
                    status: Status::Error,
                    data: None,
                    error: Some("An unexpected database error occurred.".to_string()),
                }),
            )
        }
    }
}