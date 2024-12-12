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
    get_verified_programs_list_paginated(State(db), "1".to_string()).await
}

/// Handler for retrieving a paginated list of verified programs
/// 
/// # Endpoint: GET /verified-programs/:page
/// 
/// # Returns
/// * `(StatusCode, Json<VerifiedProgramListResponse>)` - Status code and list of verified program addresses
pub(crate) async fn get_verified_programs_list_paginated(
    State(db): State<DbClient>,
    page: String,
) -> (StatusCode, Json<VerifiedProgramListResponse>) {
    // Parse page to i64
    let page = page.parse::<i64>().unwrap_or(1);

    let verified_programs = match db.get_verified_program_ids_page(page).await {
        Ok(programs) => {
            info!("Found {} verified programs", programs.len());
            programs
        }
        Err(err) => {
            error!("Failed to fetch verified programs: {}", err);
            return (StatusCode::OK, Json(VerifiedProgramListResponse { verified_programs: Vec::new() }));
        }
    };

    (StatusCode::OK, Json(VerifiedProgramListResponse { verified_programs }))
}