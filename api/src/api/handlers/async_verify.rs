use crate::{
    db::{
        models::{
            ApiResponse, JobStatus, SolanaProgramBuildParams,
            SolanaProgramBuildParamsWithSigner, VerifyResponse,
        },
        DbClient,
    },
    services::{
        onchain::program_metadata_retriever::is_program_buffer_missing,
        verification::{check_and_handle_duplicates, process_verification_request},
    },
};
use super::verify_helpers::{create_and_insert_build, setup_verification};
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

    match setup_verification(&db, &payload.program_id, None).await {
        Ok(setup) => process_verification(db, setup.params, setup.signer).await,
        Err(error_response) => error_response,
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

    match setup_verification(&db, &payload.program_id, Some(payload.signer)).await {
        Ok(setup) => process_verification(db, setup.params, setup.signer).await,
        Err(error_response) => error_response,
    }
}

/// Processes the verification request asynchronously
pub async fn process_verification(
    db: DbClient,
    payload: SolanaProgramBuildParams,
    signer: String,
) -> (StatusCode, Json<ApiResponse>) {
    // Check for existing verification
    if let Some(response) = check_and_handle_duplicates(&payload, signer.clone(), &db).await {
        check_program_closed(&db, &payload.program_id).await;
        return (StatusCode::OK, Json(response.into()));
    }

    // Create initial build record and mark it as completed
    let initial_uuid = match create_and_insert_build(&db, &payload, &signer).await {
        Ok(uuid) => uuid,
        Err(error_response) => return error_response,
    };

    if let Err(e) = db.update_build_status(&initial_uuid, JobStatus::Completed).await {
        error!("Failed to update build status to completed: {:?}", e);
    }

    // Create new build record for the actual verification
    let verification_uuid = match create_and_insert_build(&db, &payload, &signer).await {
        Ok(uuid) => uuid,
        Err(error_response) => return error_response,
    };

    // Spawn async verification task
    spawn_verification_task(db.clone(), payload, verification_uuid.clone()).await;

    // Return response with request ID
    (
        StatusCode::OK,
        Json(
            VerifyResponse {
                status: JobStatus::InProgress,
                request_id: verification_uuid,
                message: "Build verification started".to_string(),
            }
            .into(),
        ),
    )
}

/// Spawns an asynchronous verification task
async fn spawn_verification_task(db: DbClient, payload: SolanaProgramBuildParams, uuid: String) {
    info!("Verification task spawned with UUID: {}", uuid);
    tokio::spawn(async move {
        info!("Spawning verification task with uuid: {}", uuid);
        if let Err(e) = process_verification_request(payload, &uuid, &db).await {
            error!("Verification task failed: {:?}", e);
        }
    });
}

/// Checks if the program's buffer account is missing, and if so,
/// marks the program as unverified in the database.
pub async fn check_program_closed(db: &DbClient, program_id: &str) {
    if is_program_buffer_missing(program_id).await {
        info!(
            "Program {} buffer missing. Marking as unverified.",
            program_id
        );

        if let Err(e) = db.mark_program_unverified(program_id).await {
            error!(
                "Program {} buffer missing. failed to mark as unverified: {:?}",
                program_id, e
            );
        }
    }
}
