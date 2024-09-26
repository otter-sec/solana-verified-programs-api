use crate::services::onchain::OtterBuildParams;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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

impl From<OtterBuildParams> for SolanaProgramBuildParams {
    fn from(otter: OtterBuildParams) -> Self {
        SolanaProgramBuildParams {
            repository: otter.git_url.clone(),
            program_id: otter.address.to_string(),
            commit_hash: Some(otter.commit.clone()),
            lib_name: otter.get_library_name(),
            bpf_flag: Some(otter.is_bpf()),
            base_image: otter.get_base_image(),
            mount_path: otter.get_mount_path(),
            cargo_args: otter.get_cargo_args(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct VerificationStatusParams {
    pub address: String,
}
