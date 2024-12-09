use crate::db::{models::Status, DbClient};
use axum::{extract::State, http::StatusCode, Json};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

/// Response structure for individual program status
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedProgramStatusResponse {
    /// Program identifier
    pub program_id: String,
    /// Current verification status
    pub is_verified: bool,
    /// Status message
    pub message: String,
    /// Hash of the program on chain
    pub on_chain_hash: String,
    /// Hash of the executable
    pub executable_hash: String,
    /// Last verification timestamp
    pub last_verified_at: Option<NaiveDateTime>,
    /// Repository URL
    pub repo_url: String,
    /// Git commit hash
    pub commit: String,
}

/// Response structure for list of program statuses
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedProgramsStatusListResponse {
    /// Operation status
    pub status: Status,
    /// List of program statuses
    pub data: Option<Vec<VerifiedProgramStatusResponse>>,
    /// Error message if any
    pub error: Option<String>,
}

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

    let verified_programs = match db.get_verified_programs().await {
        Ok(programs) => {
            info!("Found {} verified programs", programs.len());
            programs
        }
        Err(err) => {
            error!("Failed to fetch verified programs from database: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(VerifiedProgramsStatusListResponse {
                    status: Status::Error,
                    data: None,
                    error: Some("An unexpected database error occurred.".to_string()),
                }),
            );
        }
    };

    let mut programs_status = Vec::new();

    for program in verified_programs {
        let program_id = program.program_id.clone();
        info!("Checking verification status for program: {}", program_id);

        match db.clone().check_is_verified(program_id.clone(), None).await {
            Ok(result) => {
                let status_message = if result.is_verified {
                    "On chain program verified"
                } else {
                    "On chain program not verified"
                };

                info!("Program {} status: {}", program_id, status_message);
                programs_status.push(VerifiedProgramStatusResponse {
                    program_id,
                    is_verified: result.is_verified,
                    message: status_message.to_string(),
                    on_chain_hash: result.on_chain_hash,
                    executable_hash: result.executable_hash,
                    last_verified_at: result.last_verified_at,
                    repo_url: result.repo_url,
                    commit: result.commit,
                });
            }
            Err(err) => {
                error!(
                    "Failed to get verification status for program {}: {}",
                    program_id, err
                );
                continue;
            }
        }
    }

    info!("Successfully retrieved status for all programs");
    (
        StatusCode::OK,
        Json(VerifiedProgramsStatusListResponse {
            status: Status::Success,
            data: Some(programs_status),
            error: None,
        }),
    )
}
