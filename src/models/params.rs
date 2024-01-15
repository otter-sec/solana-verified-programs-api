use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct VerificationStatusParams {
    pub address: String,
}
