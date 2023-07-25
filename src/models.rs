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
    pub bpf_flag: bool,
    pub created_at: NaiveDateTime,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SolanaProgramBuildParams {
    pub repository: String,
    pub program_id: String,
    pub commit_hash: Option<String>,
    pub lib_name: Option<String>,
    pub bpf_flag: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct VerificationStatusParams {
    pub address: String,
}

// Types for API responses

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Success,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: Status,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub is_verified: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyResponse {
    pub status: Status,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SuccessResponse {
    Status(StatusResponse),
    Verify(VerifyResponse),
}

impl From<StatusResponse> for SuccessResponse {
    fn from(value: StatusResponse) -> Self {
        Self::Status(value)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApiResponse {
    Success(SuccessResponse),
    Error(ErrorResponse),
}

impl From<StatusResponse> for ApiResponse {
    fn from(value: StatusResponse) -> Self {
        Self::Success(SuccessResponse::Status(value))
    }
}

impl From<VerifyResponse> for ApiResponse {
    fn from(value: VerifyResponse) -> Self {
        Self::Success(SuccessResponse::Verify(value))
    }
}

impl From<ErrorResponse> for ApiResponse {
    fn from(value: ErrorResponse) -> Self {
        Self::Error(value)
    }
}
