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
    pub created_at: NaiveDateTime,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Insertable, Identifiable, Queryable, AsChangeset,
)]
#[diesel(table_name = verified_programs, primary_key(id))]
pub struct VerfiedProgram {
    pub id: String,
    pub program_id: String,
    pub is_verified: bool,
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
