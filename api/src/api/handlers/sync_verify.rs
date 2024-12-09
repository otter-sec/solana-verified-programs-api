use crate::{
    db::models::{
        ApiResponse, ErrorResponse, SolanaProgramBuild, SolanaProgramBuildParams, Status,
        StatusResponse, DEFAULT_SIGNER,
    },
    db::DbClient,
    errors::ErrorMessages,
    services::verification::{check_and_handle_duplicates, process_verification_request},
};
use axum::{extract::State, http::StatusCode, Json};
use tracing::{error, info};

/// Handler for synchronous program verification
///
/// # Endpoint: POST /verify/sync
///
/// # Arguments
/// * `db` - Database client from application state
/// * `payload` - Build parameters for verification
///
/// # Returns
/// * `(StatusCode, Json<ApiResponse>)` - Status code and verification response
///
/// This endpoint performs verification synchronously, meaning it will wait for
/// the verification process to complete before returning a response.
pub(crate) async fn process_sync_verification(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> (StatusCode, Json<ApiResponse>) {
    info!(
        "Starting synchronous verification for program: {}",
        payload.program_id
    );

    // Create build record
    let verify_build_data = SolanaProgramBuild::from(&payload);
    let uuid = verify_build_data.id.clone();

    // Check for existing verification
    if let Some(response) = check_for_duplicate(&payload, &db).await {
        return response;
    }

    // Insert build parameters
    if let Err(e) = insert_build_params(&db, &verify_build_data).await {
        return e;
    }

    // Process verification
    match process_verification(&payload, &uuid, &db).await {
        Ok(response) => response,
        Err(response) => response,
    }
}

/// Checks if the program is already verified
async fn check_for_duplicate(
    payload: &SolanaProgramBuildParams,
    db: &DbClient,
) -> Option<(StatusCode, Json<ApiResponse>)> {
    match check_and_handle_duplicates(payload, DEFAULT_SIGNER.to_string(), db).await {
        Some(response) => {
            info!(
                "Found existing verification for program: {}",
                payload.program_id
            );
            Some((StatusCode::OK, Json(response.into())))
        }
        None => None,
    }
}

/// Inserts build parameters into the database
async fn insert_build_params(
    db: &DbClient,
    build_data: &SolanaProgramBuild,
) -> Result<(), (StatusCode, Json<ApiResponse>)> {
    if let Err(e) = db.insert_build_params(build_data).await {
        error!("Failed to insert build parameters: {:?}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                ErrorResponse {
                    status: Status::Error,
                    error: ErrorMessages::DB.to_string(),
                }
                .into(),
            ),
        ));
    }
    Ok(())
}

/// Processes the verification request
async fn process_verification(
    payload: &SolanaProgramBuildParams,
    uuid: &str,
    db: &DbClient,
) -> Result<(StatusCode, Json<ApiResponse>), (StatusCode, Json<ApiResponse>)> {
    match process_verification_request(payload.clone(), uuid, db).await {
        Ok(res) => {
            info!(
                "Verification completed for program: {} (verified: {})",
                payload.program_id, res.is_verified
            );

            let repo_url = payload
                .commit_hash
                .as_ref()
                .map_or(payload.repository.clone(), |hash| {
                    format!("{}/tree/{}", payload.repository, hash)
                });

            Ok((
                StatusCode::OK,
                Json(
                    StatusResponse {
                        is_verified: res.is_verified,
                        message: if res.is_verified {
                            "On chain program verified"
                        } else {
                            "On chain program not verified"
                        }
                        .to_string(),
                        on_chain_hash: res.on_chain_hash,
                        executable_hash: res.executable_hash,
                        last_verified_at: Some(res.verified_at),
                        repo_url,
                        commit: payload.commit_hash.clone().unwrap_or_default(),
                    }
                    .into(),
                ),
            ))
        }
        Err(err) => {
            error!("Verification failed: {:?}", err);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    ErrorResponse {
                        status: Status::Error,
                        error: ErrorMessages::Unexpected.to_string(),
                    }
                    .into(),
                ),
            ))
        }
    }
}
