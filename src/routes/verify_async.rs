use crate::builder::verify_build;
use crate::db::DbClient;
use crate::models::{
    ApiResponse, ErrorResponse, JobStatus, SolanaProgramBuild, SolanaProgramBuildParams, Status,
    VerifyResponse,
};
use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;

// Route handler for POST /verify which creates a new process to verify the program
pub(crate) async fn verify_async(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> (StatusCode, Json<ApiResponse>) {
    let uuid = uuid::Uuid::new_v4().to_string();
    let verify_build_data = SolanaProgramBuild {
        id: uuid.clone(),
        repository: payload.repository.clone(),
        commit_hash: payload.commit_hash.clone(),
        program_id: payload.program_id.clone(),
        lib_name: payload.lib_name.clone(),
        bpf_flag: payload.bpf_flag.unwrap_or(false),
        created_at: Utc::now().naive_utc(),
        base_docker_image: payload.base_image.clone(),
        mount_path: payload.mount_path.clone(),
        cargo_args: payload.cargo_args.clone(),
        status: JobStatus::InProgress.into(),
    };

    // Check if the build was already processed
    let is_duplicate = db.check_for_dupliate(&payload).await;

    if let Ok(respose) = is_duplicate {
        match respose.status.into() {
            JobStatus::Completed => {
                // Get the verified build from the database
                let verified_build = db.get_verified_build(&respose.program_id).await.unwrap();
                return (
                    StatusCode::OK,
                    Json(
                        VerifyResponse {
                            status: JobStatus::Completed,
                            request_id: verified_build.solana_build_id,
                            message: "Verification already completed.".to_string(),
                        }
                        .into(),
                    ),
                );
            }
            JobStatus::InProgress => {
                // Return ID to user to check status
                return (
                    StatusCode::OK,
                    Json(
                        VerifyResponse {
                            status: JobStatus::InProgress,
                            request_id: respose.id,
                            message: "Build verification already in progress".to_string(),
                        }
                        .into(),
                    ),
                );
            }
            JobStatus::Failed => {
                // Return error to user
                return (
                    StatusCode::CONFLICT,
                    Json(
                        ErrorResponse {
                            status: Status::Error,
                            error: "The previous request has already been processed, but unfortunately, the verification process has failed.".to_string(),
                        }
                        .into(),
                    ),
                );
            }
        }
    }

    // insert into database
    if let Err(e) = db.insert_build_params(&verify_build_data).await {
        tracing::error!("Error inserting into database: {:?}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                ErrorResponse {
                    status: Status::Error,
                    error: "An unforeseen database error has occurred, preventing the initiation of the build process. Kindly try again after some time.".to_string(),
                }
                .into(),
            ),
        );
    }

    tracing::info!("Inserted into database");

    //run task in background
    tokio::spawn(async move {
        match verify_build(payload, &verify_build_data.id).await {
            Ok(res) => {
                let _ = db.insert_or_update_verified_build(&res).await;
                let _ = db
                    .update_build_status(&verify_build_data.id, JobStatus::Completed.into())
                    .await;
            }
            Err(err) => {
                let _ = db
                    .update_build_status(&verify_build_data.id, JobStatus::Failed.into())
                    .await;
                tracing::error!("Error verifying build: {:?}", err);
                tracing::error!(
                    "We encountered an unexpected error during the verification process."
                );
            }
        }
    });

    (
        StatusCode::OK,
        Json(
            VerifyResponse {
                status: JobStatus::InProgress,
                request_id: uuid,
                message: "Build verification started".to_string(),
            }
            .into(),
        ),
    )
}
