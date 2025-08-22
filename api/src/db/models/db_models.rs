use crate::schema::{build_logs, solana_program_builds, verified_programs};
use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, system_program};

use super::SolanaProgramBuildParams;

pub(crate) const DEFAULT_SIGNER: Pubkey = system_program::id();

/// Represents a Solana program build in the database
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    Insertable,
    Identifiable,
    Queryable,
    AsChangeset,
    Selectable,
    QueryableByName,
    Default,
)]
#[diesel(table_name = solana_program_builds, primary_key(id))]
pub struct SolanaProgramBuild {
    /// Unique identifier for the build
    pub id: String,
    /// Repository URL
    pub repository: String,
    /// Git commit hash
    pub commit_hash: Option<String>,
    /// Program ID
    pub program_id: String,
    /// Library name
    pub lib_name: Option<String>,
    /// Base Docker image
    pub base_docker_image: Option<String>,
    /// Mount path in container
    pub mount_path: Option<String>,
    /// Cargo build arguments
    pub cargo_args: Option<Vec<String>>,
    /// BPF compilation flag
    pub bpf_flag: bool,
    /// Build creation timestamp
    pub created_at: NaiveDateTime,
    /// Build status
    pub status: String,
    /// Signer's public key
    pub signer: Option<String>,
    /// Architecture target v0,v1,v2,etc.
    pub arch: Option<String>,
}

impl<'a> From<&'a SolanaProgramBuildParams> for SolanaProgramBuild {
    fn from(params: &'a SolanaProgramBuildParams) -> Self {
        let uuid = uuid::Uuid::new_v4().to_string();
        SolanaProgramBuild {
            id: uuid.clone(),
            repository: params.repository.clone(),
            commit_hash: params.commit_hash.clone(),
            program_id: params.program_id.clone(),
            lib_name: params.lib_name.clone(),
            bpf_flag: params.bpf_flag.unwrap_or(false),
            created_at: Utc::now().naive_utc(),
            base_docker_image: params.base_image.clone(),
            mount_path: params.mount_path.clone(),
            cargo_args: params.cargo_args.clone(),
            status: JobStatus::InProgress.into(),
            signer: Some(DEFAULT_SIGNER.to_string()),
            arch: params.arch.clone(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Insertable,
    Identifiable,
    Queryable,
    AsChangeset,
    Selectable,
    QueryableByName,
)]
#[diesel(table_name = verified_programs, primary_key(id))]
pub struct VerifiedProgram {
    /// Unique identifier
    pub id: String,
    /// Program ID
    pub program_id: String,
    /// Verification status
    pub is_verified: bool,
    /// Hash of the program on chain
    pub on_chain_hash: String,
    /// Hash of the executable
    pub executable_hash: String,
    /// Verification timestamp
    pub verified_at: NaiveDateTime,
    /// Build ID reference
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
    #[serde(rename = "un-used")]
    Unused,
}

impl From<JobStatus> for String {
    fn from(status: JobStatus) -> Self {
        match status {
            JobStatus::InProgress => "in_progress".to_string(),
            JobStatus::Completed => "completed".to_string(),
            JobStatus::Failed => "failed".to_string(),
            JobStatus::Unused => "un-used".to_string(),
        }
    }
}

impl From<String> for JobStatus {
    fn from(status: String) -> Self {
        match status.as_str() {
            "in_progress" => JobStatus::InProgress,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "un-used" => JobStatus::Unused,
            _ => panic!("Invalid job status"),
        }
    }
}

/// Represents build logs in the database
#[derive(Clone, Debug, Serialize, Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name = build_logs, primary_key(id))]
pub struct BuildLogs {
    /// Unique identifier
    pub id: String,
    /// Program address
    pub program_address: String,
    /// Log file name
    pub file_name: String,
    /// Log creation timestamp
    pub created_at: NaiveDateTime,
}

#[derive(QueryableByName)]
pub struct VerifiedBuildWithSigner {
    #[diesel(embed)]
    pub solana_program_build: SolanaProgramBuild,
    #[diesel(embed)]
    pub verified_program: Option<VerifiedProgram>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Bool>)]
    pub is_frozen: Option<bool>,
}
