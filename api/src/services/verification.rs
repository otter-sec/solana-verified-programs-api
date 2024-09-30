use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use crate::db::models::{SolanaProgramBuildParams, VerifiedProgram};
use crate::errors::ApiError;
use crate::services::misc::extract_hash;
use crate::Result;
use libc::{c_ulong, getrlimit, rlimit, setrlimit, RLIMIT_AS};
use tokio::process::Command;

pub async fn verify_build(
    payload: SolanaProgramBuildParams,
    build_id: &str,
) -> Result<VerifiedProgram> {
    tracing::info!("Verifying build..");

    let mut original_rlimit = rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let max_ram_usage_bytes: c_ulong = 2 * 1024 * 1024 * 1024;
    unsafe {
        getrlimit(RLIMIT_AS, &mut original_rlimit);
        setrlimit(
            libc::RLIMIT_AS,
            &libc::rlimit {
                rlim_cur: max_ram_usage_bytes,
                rlim_max: max_ram_usage_bytes,
            },
        );
    }

    let mut cmd = Command::new("solana-verify");
    
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd.arg("verify-from-repo").arg("-um");

    if let Some(commit) = payload.commit_hash {
        cmd.arg("--commit-hash").arg(commit);
    }

    if let Some(library_name) = payload.lib_name {
        cmd.arg("--library-name").arg(library_name);
    }

    if let Some(base_image) = payload.base_image {
        cmd.arg("--base-image").arg(base_image);
    }

    if let Some(mount_path) = payload.mount_path {
        cmd.arg("--mount-path").arg(mount_path);
    }

    if let Some(bpf_flag) = payload.bpf_flag {
        if bpf_flag {
            cmd.arg("--bpf");
        }
    }

    cmd.arg("--program-id")
        .arg(&payload.program_id)
        .arg(payload.repository);

    if let Some(cargo_args) = payload.cargo_args {
        cmd.arg("--").args(&cargo_args);
    }

    tracing::info!("Running command: {:?}", cmd);

    let mut child = cmd.spawn().expect("Failed to successfully run solana-verify command");

    // Get the stdin handle
    if let Some(mut stdin) = child.stdin.take() {
        // Send 'n' to the process
        stdin.write_all(b"n\n").await?;
    }

    let output = child.wait_with_output().await.map_err(|e| {
        tracing::error!("Error running command: {:?}", e);
        ApiError::Build("Error running command".to_string())
    })?;

    let result = String::from_utf8(output.stdout)?;
    if !output.status.success() {
        return Err(ApiError::Build(result));
    }

    let onchain_hash = extract_hash(&result, "On-chain Program Hash:").unwrap_or_default();
    let build_hash =
        extract_hash(&result, "Executable Program Hash from repo:").unwrap_or_default();

    tracing::info!(
        "{} build hash {} On chain hash {}",
        payload.program_id,
        build_hash,
        onchain_hash
    );

    let verified_build = VerifiedProgram {
        id: uuid::Uuid::new_v4().to_string(),
        program_id: payload.program_id,
        is_verified: onchain_hash == build_hash,
        on_chain_hash: onchain_hash,
        executable_hash: build_hash,
        verified_at: chrono::Utc::now().naive_utc(),
        solana_build_id: build_id.to_string(),
    };

    unsafe {
        setrlimit(RLIMIT_AS, &original_rlimit);
    }

    Ok(verified_build)
}
