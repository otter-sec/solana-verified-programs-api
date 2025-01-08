use crate::{
    db::{
        models::{
            ApiResponse, ErrorResponse, JobStatus, SolanaProgramBuild, SolanaProgramBuildParams,
            SolanaProgramBuildParamsWithSigner, Status, VerifyResponse,
        },
        DbClient,
    },
    errors::ErrorMessages,
    services::{
        onchain::{self, get_program_authority},
        verification::{check_and_handle_duplicates, process_verification_request},
    },
};
use axum::{extract::State, http::StatusCode, Json};
use tracing::{error, info};

/// Handler for asynchronous program verification
///
/// # Endpoint: POST /verify
pub(crate) async fn process_async_verification(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> (StatusCode, Json<ApiResponse>) {
    info!(
        "Starting async verification for program: {}",
        payload.program_id
    );

    // get program authority from on-chain
    let (program_authority, _is_frozen) = get_program_authority(&payload.program_id)
        .await
        .unwrap_or((None, false));

    match onchain::get_otter_verify_params(&payload.program_id, None, program_authority.clone())
        .await
    {
        Ok((params, signer)) => {
            if let Err(e) = db
                .insert_or_update_program_authority(&params.address, program_authority.as_deref())
                .await
            {
                error!("Failed to update program authority: {:?}", e);
            }
            process_verification(db, SolanaProgramBuildParams::from(params), signer).await
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

/// Handler for asynchronous program verification with a specific signer
///
/// # Endpoint: POST /verify/with-signer
pub(crate) async fn process_async_verification_with_signer(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParamsWithSigner>,
) -> (StatusCode, Json<ApiResponse>) {
    info!(
        "Starting async verification for program {} with signer {}",
        payload.program_id, payload.signer
    );

    let (program_authority, _is_frozen) = get_program_authority(&payload.program_id)
        .await
        .unwrap_or((None, false));

    match onchain::get_otter_verify_params(
        &payload.program_id,
        Some(payload.signer.clone()),
        program_authority.clone(),
    )
    .await
    {
        Ok((params, signer)) => {
            if let Err(e) = db
                .insert_or_update_program_authority(&params.address, program_authority.as_deref())
                .await
            {
                error!("Failed to update program authority: {:?}", e);
            }
            process_verification(db, SolanaProgramBuildParams::from(params), signer).await
        }
        Err(err) => {
            error!("Error fetching onchain params: {:?}", err);
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

/// Processes the verification request asynchronously
async fn process_verification(
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

    // Insert initial build params
    if let Err(e) = db.insert_build_params(&verify_build_data).await {
        error!("Error inserting into database: {:?}", e);
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

    // Update the build status to completed
    if let Err(e) = db.update_build_status(&uuid, JobStatus::Completed).await {
        error!("Failed to update build status to completed: {:?}", e);
    }

    // Create and insert new build params
    let mut new_build = SolanaProgramBuild::from(&payload);
    new_build.signer = Some(signer);
    if let Err(e) = db.insert_build_params(&new_build).await {
        error!("Error inserting into database: {:?}", e);
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

    // Spawn verification task
    let req_id = new_build.id.clone();
    tokio::spawn(async move {
        info!("Spawning verification task with uuid: {:?}", &new_build.id);
        if let Err(e) = process_verification_request(payload, &new_build.id, &db).await {
            error!("Verification task failed: {:?}", e);
        }
    });

    info!("Verification task spawned with UUID: {}", req_id);
    (
        StatusCode::OK,
        Json(
            VerifyResponse {
                status: JobStatus::InProgress,
                request_id: req_id,
                message: "Build verification started".to_string(),
            }
            .into(),
        ),
    )
}
