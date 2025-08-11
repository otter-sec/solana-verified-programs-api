use crate::services::onchain::OtterBuildParams;
use serde::{Deserialize, Serialize};

/// Parameters for Solana program build operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SolanaProgramBuildParams {
    /// GitHub repository URL
    pub repository: String,
    /// Solana program ID
    pub program_id: String,
    /// Git commit hash
    pub commit_hash: Option<String>,
    /// Library name for the program
    pub lib_name: Option<String>,
    /// Flag to indicate BPF compilation
    pub bpf_flag: Option<bool>,
    /// Base Docker image for build
    pub base_image: Option<String>,
    /// Mount path in container
    pub mount_path: Option<String>,
    /// Additional cargo build arguments
    pub cargo_args: Option<Vec<String>>,
}

/// Build parameters with associated PDA signer
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SolanaProgramBuildParamsWithSigner {
    /// Signer's public key
    pub signer: String,
    /// Solana program ID
    pub program_id: String,
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

/// Parameters for verification status requests
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct VerificationStatusParams {
    /// Program address to check
    pub address: String,
}

#[derive(Clone)]
pub struct ProgramAuthorityParams {
    pub authority: Option<String>,
    pub frozen: bool,
    pub closed: bool,
}

/// Complete program authority data from database
#[derive(Debug, Clone)]
pub struct ProgramAuthorityData {
    pub authority: Option<String>,
    pub is_frozen: bool,
    pub is_closed: bool,
}
