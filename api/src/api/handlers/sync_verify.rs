use super::verify_helpers::{create_and_insert_build, create_internal_error, setup_verification};
use crate::{
    db::{
        models::{ApiResponse, SolanaProgramBuild, SolanaProgramBuildParams, StatusResponse},
        DbClient,
    },
    services::{
        build_repository_url,
        verification::{check_and_handle_duplicates, process_verification_request},
    },
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

    match setup_verification(&db, &payload.program_id, None).await {
        Ok(setup) => process_verification_sync(db, setup.params, setup.signer).await,
        Err(error_response) => error_response,
    }
}

/// Processes the verification request synchronously
async fn process_verification_sync(
    db: DbClient,
    payload: SolanaProgramBuildParams,
    signer: String,
) -> (StatusCode, Json<ApiResponse>) {
    // Check for existing verification
    if let Some(response) = check_and_handle_duplicates(&payload, signer.clone(), &db).await {
        return (StatusCode::OK, Json(response.into()));
    }

    // Create and insert build parameters
    let uuid = match create_and_insert_build(&db, &payload, &signer).await {
        Ok(uuid) => uuid,
        Err(error_response) => return error_response,
    };

    // Process verification synchronously
    match process_verification_request(payload.clone(), &uuid, &db).await {
        Ok(res) => {
            info!(
                "Verification completed for program: {} (verified: {})",
                payload.program_id, res.is_verified
            );

            (
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
                        repo_url: {
                            let mut build = SolanaProgramBuild::from(&payload);
                            build.signer = Some(signer.clone());
                            build_repository_url(&build)
                        },
                        commit: payload.commit_hash.clone().unwrap_or_default(),
                    }
                    .into(),
                ),
            )
        }
        Err(err) => {
            error!("Verification failed: {:?}", err);
            create_internal_error()
        }
    }
}
