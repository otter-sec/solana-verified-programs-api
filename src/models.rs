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
pub struct VerifySyncResponse {
    pub status: Status,
    pub is_verified: bool,
    pub on_chain_hash: String,
    pub executable_hash: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: Status,
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationStatusResponse {
    pub is_verified: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyAsyncResponse {
    pub status: Status,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SuccessResponse {
    VerifySync(VerifySyncResponse),
    VerificationStatus(VerificationStatusResponse),
    VerifyAsync(VerifyAsyncResponse),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApiResponse {
    Success(SuccessResponse),
    Error(ErrorResponse),
}
