use crate::db::{models::VerifiedProgramListResponse, DbClient};
use axum::{extract::State, http::StatusCode, Json};
use tracing::{error, info};

/// Handler for retrieving a list of all verified programs
///
/// # Endpoint: GET /verified-programs
///
/// # Returns
/// * `(StatusCode, Json<VerifiedProgramListResponse>)` - Status code and list of verified program addresses
///
/// On success, returns OK status with the list of program IDs
/// On failure, still returns an empty list but logs the error
pub(crate) async fn get_verified_programs_list(
    State(db): State<DbClient>,
) -> (StatusCode, Json<VerifiedProgramListResponse>) {
    info!("Fetching list of verified programs");

    let verified_programs = match db.get_verified_programs().await {
        Ok(programs) => {
            info!("Found {} verified programs", programs.len());
            programs
        }
        Err(err) => {
            error!("Failed to fetch verified programs: {}", err);
            return (
                StatusCode::OK,
                Json(VerifiedProgramListResponse {
                    verified_programs: Vec::new(),
                }),
            );
        }
    };

    // Extract program IDs from the verified programs
    let programs_list = verified_programs
        .iter()
        .map(|program| program.program_id.clone())
        .collect::<Vec<String>>();

    info!("Successfully retrieved verified programs list");
    (
        StatusCode::OK,
        Json(VerifiedProgramListResponse {
            verified_programs: programs_list,
        }),
    )
}
