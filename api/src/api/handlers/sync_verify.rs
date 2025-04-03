use crate::{
    db::{
        models::{
            ApiResponse, ErrorResponse, SolanaProgramBuild, SolanaProgramBuildParams, Status,
            StatusResponse,
        },
        DbClient,
    },
    errors::ErrorMessages,
    services::{
        build_repository_url,
        onchain::{self, get_program_authority},
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

    // get program authority from on-chain
    let (program_authority, is_frozen) = get_program_authority(&payload.program_id)
        .await
        .unwrap_or((None, false));

    match onchain::get_otter_verify_params(&payload.program_id, None, program_authority.clone())
        .await
    {
        Ok((params, signer)) => {
            if let Err(err) = db
                .insert_or_update_program_authority(
                    &params.address,
                    program_authority.as_deref(),
                    is_frozen,
                )
                .await
            {
                error!("Failed to update program authority: {:?}", err);
            }
            process_verification_sync(db, SolanaProgramBuildParams::from(params), signer).await
        }
        Err(err) => {
            error!(
                "Unable to find on-chain PDA for given program id: {:?}",
                err
            );
            (
                StatusCode::NOT_FOUND,
                Json(
                    ErrorResponse {
                        status: Status::Error,
                        error: ErrorMessages::NoPDA.to_string(),
                    }
                    .into(),
                ),
            )
        }
    }
}

/// Processes the verification request synchronously
async fn process_verification_sync(
    db: DbClient,
    payload: SolanaProgramBuildParams,
    signer: String,
) -> (StatusCode, Json<ApiResponse>) {
    let mut verify_build_data = SolanaProgramBuild::from(&payload);
    verify_build_data.signer = Some(signer.clone());
    let uuid = verify_build_data.id.clone();

    // Check for existing verification
    if let Some(response) = check_and_handle_duplicates(&payload, signer.clone(), &db).await {
        return (StatusCode::OK, Json(response.into()));
    }

    // Insert build parameters
    if let Err(e) = db.insert_build_params(&verify_build_data).await {
        error!("Failed to insert build parameters: {:?}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                ErrorResponse {
                    status: Status::Error,
                    error: ErrorMessages::DB.to_string(),
                }
                .into(),
            ),
        );
    }

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
                        repo_url: build_repository_url(&verify_build_data),
                        commit: payload.commit_hash.clone().unwrap_or_default(),
                    }
                    .into(),
                ),
            )
        }
        Err(err) => {
            error!("Verification failed: {:?}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    ErrorResponse {
                        status: Status::Error,
                        error: ErrorMessages::Unexpected.to_string(),
                    }
                    .into(),
                ),
            )
        }
    }
}
