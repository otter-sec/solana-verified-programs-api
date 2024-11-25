use crate::db::models::{
    ApiResponse, ErrorResponse, JobStatus, SolanaProgramBuild, SolanaProgramBuildParams, Status,
    VerifyResponse,
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
    let mut payload = payload;
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
    let params_from_onchain = onchain::get_otter_verify_params(&verify_build_data.program_id).await;

    if let Ok(params_from_onchain) = params_from_onchain {
        tracing::info!("{:?} using Otter params", params_from_onchain);
        payload = SolanaProgramBuildParams::from(params_from_onchain);

        // check if the params was already processed
        let is_duplicate = check_and_handle_duplicates(&payload, &db).await;
        if let Some(respose) = is_duplicate {
            return (StatusCode::OK, Json(respose.into()));
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

    //run task in background
    let req_id = uuid.clone();
    tokio::spawn(async move {
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
