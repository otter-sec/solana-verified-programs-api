use crate::api::responses::build_repository_url;
use crate::api::responses::{JobReplyStatus, JobStatus, JobVerificationResponse};
use crate::db::DbClient;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use tracing::{error, info};
use uuid::Uuid;

/// `GET /job/{job_id}` -- verification status for an async build.
pub(crate) async fn get_job_status(
    State(db): State<DbClient>,
    Path(job_id): Path<String>,
) -> (StatusCode, Json<JobVerificationResponse>) {
    info!("Checking status for job: {}", job_id);

    let job = match Uuid::parse_str(&job_id).ok().map(|id| db.get_build(id)) {
        Some(fut) => match fut.await {
            Ok(Some(b)) => b,
            Ok(None) => return reply(JobReplyStatus::Unknown, "Job not found"),
            Err(e) => {
                error!("Failed to get job status from database: {}", e);
                return reply(
                    JobReplyStatus::Unknown,
                    "Unexpected error while getting Data from DB",
                );
            }
        },
        None => return reply(JobReplyStatus::Unknown, "Invalid job id (expected UUID)"),
    };

    match job.status {
        JobStatus::Completed => {
            let cached_hash = db
                .cached_on_chain_hash(&job.program_id)
                .await
                .ok()
                .flatten()
                .unwrap_or_default();
            (
                StatusCode::OK,
                Json(JobVerificationResponse {
                    status: JobReplyStatus::Completed,
                    message: "Job completed".to_string(),
                    on_chain_hash: cached_hash,
                    executable_hash: job.executable_hash.unwrap_or_default(),
                    repo_url: build_repository_url(&job.repository, job.commit_hash.as_deref()),
                }),
            )
        }
        JobStatus::Failed => reply(JobReplyStatus::Failed, "Verification failed"),
        JobStatus::InProgress => reply(
            JobReplyStatus::InProgress,
            "Please wait, the verification is in progress",
        ),
    }
}

fn reply(status: JobReplyStatus, message: &str) -> (StatusCode, Json<JobVerificationResponse>) {
    (
        StatusCode::OK,
        Json(JobVerificationResponse {
            status,
            message: message.to_string(),
            on_chain_hash: String::new(),
            executable_hash: String::new(),
            repo_url: String::new(),
        }),
    )
}
