use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildCommandArgs {
    pub repo: String,
    pub program_id: String,
    pub command: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SolanaProgramBuildParams {
    pub repository: String,
    pub program_id: String,
    pub commit_hash: Option<String>,
    pub lib_name: Option<String>,
    pub bpf_flag: Option<bool>,
    pub base_image: Option<String>,
    pub mount_path: Option<String>,
    pub cargo_args: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Success,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyResponse {
    pub status: JobStatus,
    pub request_id: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub is_verified: bool,
    pub message: String,
    pub on_chain_hash: String,
    pub executable_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: Status,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobResponse {
    pub status: JobStatus,
    pub respose: Option<JobVerificationResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum JobStatus {
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobVerificationResponse {
    pub status: JobStatus,
    pub message: String,
    pub on_chain_hash: String,
    pub executable_hash: String,
    pub repo_url: String,
}
