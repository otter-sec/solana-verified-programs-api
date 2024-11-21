use crate::db::models::Status;
use crate::db::DbClient;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedProgramStatusResponse {
    pub program_id: String,
    pub is_verified: bool,
    pub message: String,
    pub on_chain_hash: String,
    pub executable_hash: String,
    pub last_verified_at: Option<chrono::NaiveDateTime>,
    pub repo_url: String,
    pub commit: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedProgramsStatusListResponse {
    pub status: Status,
    pub data: Option<Vec<VerifiedProgramStatusResponse>>,
    pub error: Option<String>,
}

pub(crate) async fn get_verified_programs_status(
    State(db): State<DbClient>,
) -> (StatusCode, Json<VerifiedProgramsStatusListResponse>) {

    let verified_programs = match db.get_verified_programs().await {
        Ok(programs) => programs,
        Err(err) => {
            tracing::error!("Error getting verified programs from database: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(VerifiedProgramsStatusListResponse {
                    status: Status::Error,
                    data: None,
                    error: Some("An unexpected database error occurred.".to_string()),
                })
            );
        }
    };

    let mut programs_status = Vec::new();

    for program in verified_programs {
        // db.clone() in this case is essentially free
        // No performance penalty for using it in loops or async operations
        match db.clone().check_is_verified(program.program_id.clone()).await {
            Ok(result) => {
                programs_status.push(VerifiedProgramStatusResponse {
                    program_id: program.program_id,
                    is_verified: result.is_verified,
                    message: if result.is_verified {
                        "On chain program verified".to_string()
                    } else {
                        "On chain program not verified".to_string()
                    },
                    on_chain_hash: result.on_chain_hash,
                    executable_hash: result.executable_hash,
                    last_verified_at: result.last_verified_at,
                    repo_url: result.repo_url,
                    commit: result.commit,
                });
            }
            Err(err) => {
                tracing::error!("Error getting verification status: {}", err);
                continue;
            }
        }
    }

    (
        StatusCode::OK,
        Json(VerifiedProgramsStatusListResponse {
            status: Status::Success,
            data: Some(programs_status),
            error: None,
        })
    )
}