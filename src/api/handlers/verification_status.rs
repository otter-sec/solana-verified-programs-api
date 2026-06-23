use crate::api::responses::{
    ApiResponse, ErrorResponse, ExtendedStatusResponse, Status, StatusResponse, SuccessResponse,
    VerificationStatusParams,
};
use crate::db::DbClient;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use tracing::{error, info};

/// Handler for checking if a specific program is verified
///
/// # Endpoint: GET /status/{address}
pub(crate) async fn get_verification_status(
    State(db): State<DbClient>,
    Path(VerificationStatusParams { address }): Path<VerificationStatusParams>,
) -> Response {
    info!("Checking verification status for program: {}", address);

    match db.check_is_verified(address).await {
        Ok(json) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        Err(err) => {
            error!("Failed to check verification status: {}", err);
            (
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
            )
                .into_response()
        }
    }
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
