use crate::db::models::{
    ApiResponse, ErrorResponse, Status, StatusResponse, SuccessResponse, VerificationStatusParams,
};
use crate::db::DbClient;
use axum::extract::{Path, State};
use axum::Json;
use tracing::{error, info};

/// Handler for checking if a specific program is verified
///
/// # Endpoint: GET /status/:address
///
/// # Arguments
/// * `db` - Database client from application state
/// * `address` - Program address to check verification status
///
/// # Returns
/// * `Json<ApiResponse>` - Verification status and details of the program
pub(crate) async fn get_verification_status(
    State(db): State<DbClient>,
    Path(VerificationStatusParams { address }): Path<VerificationStatusParams>,
) -> Json<ApiResponse> {
    info!("Checking verification status for program: {}", address);

    match db.check_is_verified(address, None).await {
        Ok(result) => {
            let status_message = if result.is_verified {
                "On chain program verified"
            } else {
                "On chain program not verified"
            };

            info!(
                "Program {} status: {} (verified: {})",
                result.on_chain_hash, status_message, result.is_verified
            );

            Json(
                StatusResponse {
                    is_verified: result.is_verified,
                    message: status_message.to_string(),
                    on_chain_hash: result.on_chain_hash,
                    last_verified_at: result.last_verified_at,
                    executable_hash: result.executable_hash,
                    repo_url: result.repo_url,
                    commit: result.commit,
                }
                .into(),
            )
        }
        Err(_) => {
            Json(
                StatusResponse {
                    is_verified: false,
                    message: "On chain program not verified".to_string(),
                    on_chain_hash: String::new(),
                    last_verified_at: None,
                    executable_hash: String::new(),
                    repo_url: String::new(),
                    commit: String::new(),
                }
                .into(),
            )
        }
    }
}

/// Handler for retrieving all verification information for a program
///
/// # Endpoint: GET /status/:address/all
///
/// # Arguments
/// * `db` - Database client from application state
/// * `address` - Program address to get verification information
///
/// # Returns
/// * `Json<ApiResponse>` - All verification information for the program
pub(crate) async fn get_verification_status_all(
    State(db): State<DbClient>,
    Path(VerificationStatusParams { address }): Path<VerificationStatusParams>,
) -> Json<ApiResponse> {
    info!(
        "Fetching all verification information for program: {}",
        address
    );

    match db.get_all_verification_info(address).await {
        Ok(result) => {
            info!("Successfully retrieved all verification info");
            Json(ApiResponse::Success(SuccessResponse::StatusAll(result)))
        }
        Err(err) => {
            error!(
                "Failed to get verification information from database: {}",
                err
            );
            Json(
                ErrorResponse {
                    status: Status::Error,
                    error: "An unexpected database error occurred.".to_string(),
                }
                .into(),
            )
        }
    }
}
