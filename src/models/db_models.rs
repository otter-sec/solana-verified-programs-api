use crate::schema::{solana_program_builds, verified_programs};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Insertable, Identifiable, Queryable, AsChangeset)]
#[diesel(table_name = solana_program_builds, primary_key(id))]
pub struct SolanaProgramBuild {
    pub id: String,
    pub repository: String,
    pub commit_hash: Option<String>,
    pub program_id: String,
    pub lib_name: Option<String>,
    pub base_docker_image: Option<String>,
    pub mount_path: Option<String>,
    pub cargo_args: Option<Vec<String>>,
    pub bpf_flag: bool,
    pub created_at: NaiveDateTime,
    pub status: String,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Insertable, Identifiable, Queryable, AsChangeset,
)]
#[diesel(table_name = verified_programs, primary_key(id))]
pub struct VerifiedProgram {
    pub id: String,
    pub program_id: String,
    pub is_verified: bool,
    pub on_chain_hash: String,
    pub executable_hash: String,
    pub verified_at: NaiveDateTime,
    pub solana_build_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum JobStatus {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

impl From<JobStatus> for String {
    fn from(status: JobStatus) -> Self {
        match status {
            JobStatus::InProgress => "in_progress".to_string(),
            JobStatus::Completed => "completed".to_string(),
            JobStatus::Failed => "failed".to_string(),
        }
    }
}

impl From<String> for JobStatus {
    fn from(status: String) -> Self {
        match status.as_str() {
            "in_progress" => JobStatus::InProgress,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            _ => panic!("Invalid job status"),
        }
    }
}
