use crate::db::DbClient;
use crate::responses::{
    ApiResponse, ErrorResponse, ExtendedStatusResponse, Status, StatusResponse, SuccessResponse,
    VerificationStatusParams,
};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use tracing::{error, info};

/// Handler for checking if a specific program is verified
///
/// # Endpoint: GET /status/{address}
///
/// # Arguments
/// * `db` - Database client from application state
/// * `address` - Program address to check verification status
///
/// # Returns
/// * `(StatusCode, Json<ExtendedStatusResponse>)` - HTTP status and verification status details
pub(crate) async fn get_verification_status(
    State(db): State<DbClient>,
    Path(VerificationStatusParams { address }): Path<VerificationStatusParams>,
) -> (StatusCode, Json<ExtendedStatusResponse>) {
    info!("Checking verification status for program: {}", address);

    let r = match db.check_is_verified(address).await {
        Ok(r) => r,
        Err(err) => {
            error!("Failed to check verification status: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ExtendedStatusResponse {
                    status: StatusResponse {
                        is_verified: false,
                        message: "Failed to check verification status".to_string(),
                        on_chain_hash: String::new(),
                        last_verified_at: None,
                        executable_hash: String::new(),
                        repo_url: String::new(),
                        commit: String::new(),
                    },
                    is_frozen: false,
                    is_closed: false,
                }),
            );
        }
    };
    let message = if r.is_verified {
        "On chain program verified"
    } else {
        "On chain program not verified"
    };
    info!("Program status: {} (verified: {})", message, r.is_verified);

    (
        StatusCode::OK,
        Json(ExtendedStatusResponse {
            status: StatusResponse {
                is_verified: r.is_verified,
                message: message.to_string(),
                on_chain_hash: r.on_chain_hash,
                last_verified_at: r.last_verified_at,
                executable_hash: r.executable_hash,
                repo_url: r.repo_url,
                commit: r.commit,
            },
            is_frozen: r.is_frozen,
            is_closed: r.is_closed,
        }),
    )
}

/// Handler for retrieving all verification information for a program
///
/// # Endpoint: GET /status-all/{address}
///
/// # Arguments
/// * `db` - Database client from application state
/// * `address` - Program address to get verification information
///
/// # Returns
/// * `(StatusCode, Json<ApiResponse>)` - HTTP status and all verification information
pub(crate) async fn get_verification_status_all(
    State(db): State<DbClient>,
    Path(VerificationStatusParams { address }): Path<VerificationStatusParams>,
) -> (StatusCode, Json<ApiResponse>) {
    info!(
        "Fetching all verification information for program: {}",
        address
    );

    match db.get_all_verification_info(address).await {
        Ok(result) => {
            info!("Successfully retrieved all verification info");
            (
                StatusCode::OK,
                Json(ApiResponse::Success(SuccessResponse::StatusAll(result))),
            )
        }
        Err(err) => {
            error!(
                "Failed to get verification information from database: {}",
                err
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    ErrorResponse {
                        status: Status::Error,
                        error: "An unexpected database error occurred.".to_string(),
                    }
                    .into(),
                ),
            )
        }
    }
}
