use crate::db::models::{
    ApiResponse, ErrorResponse, JobStatus, SolanaProgramBuild, SolanaProgramBuildParams,
    SolanaProgramBuildParamsWithSigner, Status, VerifyResponse,
};
use crate::db::DbClient;
use crate::errors::ErrorMessages;
use crate::services::onchain;
use crate::services::verification::{check_and_handle_duplicates, check_and_process_verification};
use axum::{extract::State, http::StatusCode, Json};

// Route handler for POST /verify which creates a new process to verify the program
pub(crate) async fn process_async_verification(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> (StatusCode, Json<ApiResponse>) {
    let params_from_onchain = onchain::get_otter_verify_params(&payload.program_id, None)
        .await
        .map_err(|err| {
            tracing::error!(
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
        });

    match params_from_onchain {
        Ok((params, signer)) => {
            process_verification(db, SolanaProgramBuildParams::from(params), signer).await
        }
        Err(e) => e,
    }
}

pub(crate) async fn process_async_verification_with_signer(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParamsWithSigner>,
) -> (StatusCode, Json<ApiResponse>) {
    let program_id = payload.program_id.clone();
    let signer = payload.signer.clone();
    let params_from_onchain =
        onchain::get_otter_verify_params(&program_id, Some(signer.clone())).await;

    match params_from_onchain {
        Ok((params, _)) => {
            process_verification(db, SolanaProgramBuildParams::from(params), signer).await
        }
        Err(e) => {
            tracing::error!("Error fetching onchain params: {:?}", e);
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

async fn process_verification(
    db: DbClient,
    payload: SolanaProgramBuildParams,
    signer: String,
) -> (StatusCode, Json<ApiResponse>) {
    let mut verify_build_data = SolanaProgramBuild::from(&payload);
    verify_build_data.signer = Some(signer.clone());

    let mut uuid = verify_build_data.id.clone();

    // Check if the build was already processed
    if let Some(response) = check_and_handle_duplicates(&payload, signer.clone(), &db).await {
        return (StatusCode::OK, Json(response.into()));
    }

    // Else insert into database
    if let Err(e) = db.insert_build_params(&verify_build_data).await {
        tracing::error!("Error inserting into database: {:?}", e);
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

    // Updated the build status to completed for recieved build params and update the uuid to a new one
    let _ = db
        .update_build_status(&uuid, JobStatus::Completed.into())
        .await
        .map_err(|e| {
            tracing::error!("Error updating build status: {:?}", e);
            e
        });

    // Insert the new build params into the database and update the uuid
    let mut new_build = SolanaProgramBuild::from(&payload);
    new_build.signer = Some(signer.clone());
    let _ = db.insert_build_params(&new_build).await;
    uuid = new_build.id.clone();

    // Now check if there was a PDA associated with that Build if so we need to handle it
    //run task in background
    let req_id = uuid.clone();
    tokio::spawn(async move {
        tracing::info!(
            "Spawning verification task with signer: {:?} and uuid: {:?}",
            &signer,
            &uuid
        );
        let _ = check_and_process_verification(payload, &uuid, &db).await;
    });

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
