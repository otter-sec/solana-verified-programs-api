use crate::db::{
    models::{JobStatus, JobVerificationResponse},
    DbClient,
};
use axum::extract::{Path, State};
use axum::Json;
use tracing::{error, info};

/// Handler for retrieving the status of a verification job
///
/// # Endpoint: GET /job/:job_id
///
/// # Arguments
/// * `db` - Database client from application state
/// * `job_id` - Unique identifier for the verification job
///
/// # Returns
/// * `Json<JobVerificationResponse>` - Current status and details of the verification job
pub(crate) async fn get_job_status(
    State(db): State<DbClient>,
    Path(job_id): Path<String>,
) -> Json<JobVerificationResponse> {
    info!("Checking status for job: {}", job_id);

    match db.get_job(&job_id).await {
        Ok(job) => {
            let status: JobStatus = job.status.into();
            match status {
                JobStatus::Completed => {
                    info!("Job {} completed, fetching verification details", job_id);
                    match db.get_verified_build(&job.program_id, None).await {
                        Ok(verified_build) => {
                            let repo_url = job.commit_hash.map_or(job.repository.clone(), |hash| {
                                format!("{}/tree/{}", job.repository.trim_end_matches('/'), hash)
                            });

                            info!(
                                "Successfully retrieved verification details for job {}",
                                job_id
                            );
                            Json(JobVerificationResponse {
                                status: JobStatus::Completed.into(),
                                message: "Job completed".to_string(),
                                on_chain_hash: verified_build.on_chain_hash,
                                executable_hash: verified_build.executable_hash,
                                repo_url,
                            })
                        }
                        Err(err) => {
                            error!("Failed to get verification data from database: {}", err);
                            create_error_response("Unexpected error while getting Data from DB")
                        }
                    }
                }
                JobStatus::Failed => {
                    info!("Job {} failed", job_id);
                    Json(JobVerificationResponse {
                        status: JobStatus::Failed.into(),
                        message: "Verification failed".to_string(),
                        on_chain_hash: String::new(),
                        executable_hash: String::new(),
                        repo_url: String::new(),
                    })
                }
                JobStatus::InProgress => {
                    info!("Job {} is still in progress", job_id);
                    Json(JobVerificationResponse {
                        status: JobStatus::InProgress.into(),
                        message: "Please wait, the verification is in progress".to_string(),
                        on_chain_hash: String::new(),
                        executable_hash: String::new(),
                        repo_url: String::new(),
                    })
                }
                JobStatus::Unused => {
                    info!("Job {} marked as unused", job_id);
                    Json(JobVerificationResponse {
                        status: JobStatus::Failed.into(),
                        message: "These params were not used. There might be a PDA associated with this program ID.".to_string(),
                        on_chain_hash: String::new(),
                        executable_hash: String::new(),
                        repo_url: String::new(),
                    })
                }
            }
        }
        Err(err) => {
            error!("Failed to get job status from database: {}", err);
            create_error_response("Unexpected error while getting Data from DB")
        }
    }
}

/// Creates a standard error response
fn create_error_response(message: &str) -> Json<JobVerificationResponse> {
    Json(JobVerificationResponse {
        status: "unknown".to_string(),
        message: message.to_string(),
        on_chain_hash: String::new(),
        executable_hash: String::new(),
        repo_url: String::new(),
    })
}
