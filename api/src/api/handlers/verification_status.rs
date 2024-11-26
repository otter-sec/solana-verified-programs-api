use crate::db::models::{
    ApiResponse, ErrorResponse, Status, StatusResponse, SuccessResponse, VerificationStatusParams,
};
use crate::db::DbClient;
use axum::extract::{Path, State};
use axum::Json;

//  Route handler for GET /status/:address which checks if the program is verified or not
pub(crate) async fn get_verification_status(
    State(db): State<DbClient>,
    Path(VerificationStatusParams { address }): Path<VerificationStatusParams>,
) -> Json<ApiResponse> {
    match db.check_is_verified(address, None).await {
        Ok(result) => Json(
            StatusResponse {
                is_verified: result.is_verified,
                message: if result.is_verified {
                    "On chain program verified".to_string()
                } else {
                    "On chain program not verified".to_string()
                },
                on_chain_hash: result.on_chain_hash,
                last_verified_at: result.last_verified_at,
                executable_hash: result.executable_hash,
                repo_url: result.repo_url,
                commit: result.commit,
            }
            .into(),
        ),
        Err(err) => {
            tracing::error!("Error getting data from database: {}", err);
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

pub(crate) async fn get_verification_status_all(
    State(db): State<DbClient>,
    Path(VerificationStatusParams { address }): Path<VerificationStatusParams>,
) -> Json<ApiResponse> {
    match db.get_all_verification_info(address).await {
        Ok(result) => Json(ApiResponse::Success(SuccessResponse::StatusAll(result))),
        Err(err) => {
            tracing::error!("Error getting data from database: {}", err);
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
