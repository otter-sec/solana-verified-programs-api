use crate::db::DbClient;
use crate::db::models::{JobStatus, JobVerificationResponse};
use axum::extract::{Path, State};
use axum::Json;

// Route handler for GET /jobs/:job_id which checks the status of a job
pub(crate) async fn get_job_status(
    State(db): State<DbClient>,
    Path(job_id): Path<String>,
) -> Json<JobVerificationResponse> {
    let status = db.get_job(&job_id).await;
    match status {
        Ok(res) => match res.status.into() {
            JobStatus::Completed => {
                let verify_build_data = db.get_verified_build(&res.program_id).await;
                match verify_build_data {
                    Ok(verified_build) => Json(JobVerificationResponse {
                        status: JobStatus::Completed.into(),
                        message: "Job completed".to_string(),
                        on_chain_hash: verified_build.on_chain_hash,
                        executable_hash: verified_build.executable_hash,
                        repo_url: res.commit_hash.map_or(res.repository.clone(), |hash| {
                            format!("{}/commit/{}", res.repository, hash)
                        }),
                    }),
                    Err(err) => {
                        tracing::error!("Error getting data from database: {}", err);
                        Json(JobVerificationResponse {
                            status: "unknown".to_string(),
                            message: "Unexpected error while getting Data from DB".to_string(),
                            on_chain_hash: "".to_string(),
                            executable_hash: "".to_string(),
                            repo_url: "".to_string(),
                        })
                    }
                }
            }
            JobStatus::Failed => Json(JobVerificationResponse {
                status: JobStatus::Failed.into(),
                message: "Verification failed".to_string(),
                on_chain_hash: "".to_string(),
                executable_hash: "".to_string(),
                repo_url: "".to_string(),
            }),
            JobStatus::InProgress => Json(JobVerificationResponse {
                status: JobStatus::InProgress.into(),
                message: "Please wait the verification was in progress".to_string(),
                on_chain_hash: "".to_string(),
                executable_hash: "".to_string(),
                repo_url: "".to_string(),
            }),
        },
        Err(err) => {
            tracing::error!("Error getting data from database: {}", err);
            Json(JobVerificationResponse {
                status: "unknown".to_string(),
                message: "Unexpected error while getting Data from DB".to_string(),
                on_chain_hash: "".to_string(),
                executable_hash: "".to_string(),
                repo_url: "".to_string(),
            })
        }
    }
}
