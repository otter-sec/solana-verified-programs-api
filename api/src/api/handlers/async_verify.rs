use crate::db::models::{
    ApiResponse, ErrorResponse, JobStatus, SolanaProgramBuild, SolanaProgramBuildParams,
    SolanaProgramBuildParamsWithSigner, Status, VerifyResponse,
};
use crate::db::DbClient;
use crate::errors::ErrorMessages;
use crate::services::onchain;
use crate::services::verification::{check_and_handle_duplicates, check_and_process_verification};
use axum::{extract::State, http::StatusCode, Json};
use solana_sdk::system_program;

// Route handler for POST /verify which creates a new process to verify the program
pub(crate) async fn process_async_verification(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> (StatusCode, Json<ApiResponse>) {
    process_verification(db, payload, None).await
}

pub(crate) async fn process_async_verification_with_signer(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParamsWithSigner>,
) -> (StatusCode, Json<ApiResponse>) {
    process_verification(db, payload.params, Some(payload.signer)).await
}

async fn process_verification(
    db: DbClient,
    payload: SolanaProgramBuildParams,
    signer: Option<String>,
) -> (StatusCode, Json<ApiResponse>) {
    let mut payload = SolanaProgramBuildParamsWithSigner {
        params: payload,
        signer: signer.clone().unwrap_or(system_program::id().to_string()),
    };
    let verify_build_data = SolanaProgramBuild::from(&payload);
    let mut uuid = verify_build_data.id.clone();

    // Check if the build was already processed
    let is_dublicate = check_and_handle_duplicates(&payload, &db).await;

    if let Some(response) = is_dublicate {
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

    // Now check if there was a PDA associated with that Build if so we need to handle it
    let params_from_onchain =
        onchain::get_otter_verify_params(&verify_build_data.program_id, signer.clone()).await;

    match params_from_onchain {
        Ok(params) => {
            tracing::info!("{:?} using Otter params", params);
            payload.params = SolanaProgramBuildParams::from(params);

            // check if the params was already processed
            let is_duplicate = check_and_handle_duplicates(&payload, &db).await;
            if let Some(response) = is_duplicate {
                return (StatusCode::OK, Json(response.into()));
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
            let new_build = SolanaProgramBuild::from(&payload);
            let _ = db.insert_build_params(&new_build).await;
            uuid = new_build.id.clone();
        }
        Err(e) => {
            if signer.is_some() {
                tracing::error!("Error fetching onchain params: {:?}", e);
                return (
                    StatusCode::OK,
                    Json(
                        ErrorResponse {
                            status: Status::Error,
                            error: ErrorMessages::NoPDA.to_string(),
                        }
                        .into(),
                    ),
                );
            }
        }
    }

    //run task in background
    let req_id = uuid.clone();
    tokio::spawn(async move {
        tracing::info!(
            "Spawning verification task with signer: {:?} and uuid: {:?}",
            signer,
            uuid
        );
        let _ = check_and_process_verification(payload.params, &uuid, &db).await;
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
