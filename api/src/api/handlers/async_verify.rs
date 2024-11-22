use crate::db::models::{
    ApiResponse, ErrorResponse, JobStatus, SolanaProgramBuild, SolanaProgramBuildParams, Status,
    VerifyResponse,
};
use crate::db::DbClient;
use crate::errors::ErrorMessages;
use crate::services::verification::check_and_process_verification;
use axum::{extract::State, http::StatusCode, Json};

// Route handler for POST /verify which creates a new process to verify the program
pub(crate) async fn process_async_verification(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> (StatusCode, Json<ApiResponse>) {
    let verify_build_data = SolanaProgramBuild::from(&payload);
    let uuid = verify_build_data.id.clone();

    // Check if the build was already processed
    let is_duplicate = db.check_for_duplicate(&payload).await;

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
                // Retry build
                tracing::info!("Previous build failed for this program. Initiating new build");
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
                    error: ErrorMessages::DB.to_string(),
                }
                .into(),
            ),
        );
    }

    tracing::info!("Inserted into database");

    let req_id = uuid.clone();
    //run task in background
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
