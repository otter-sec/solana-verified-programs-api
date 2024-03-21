use tokio::process::Command;

use crate::errors::ApiError;
use crate::models::{SolanaProgramBuild, SolanaProgramBuildParams, VerifiedProgram};
use crate::Result;
use libc::{c_ulong, getrlimit, rlimit, setrlimit, RLIMIT_AS};

fn get_last_line(output: &str) -> Option<String> {
    output.lines().last().map(ToOwned::to_owned)
}

/// Create a URL for the repository of the program
/// Arguments:
/// * `res`: The `res` parameter is a `SolanaProgramBuild` struct that contains the repository
/// and the commit hash of the program.
/// Returns: A string that represents the URL of the repository.
///
pub fn get_repo_url(build_params: &SolanaProgramBuild) -> String {
    build_params.commit_hash.as_ref().map_or_else(
        || build_params.repository.clone(),
        |hash| format!("{}/commit/{}", build_params.repository, hash),
    )
}

fn extract_hash(output: &str, prefix: &str) -> Option<String> {
    output
        .lines()
        .find(|line| line.starts_with(prefix))
        .map(|line| {
            let hash = line.trim_start_matches(prefix.trim()).trim();
            hash.to_owned()
        })
}

/// The `verify_build` function verifies a Solana program build by executing the `solana-verify` command
/// and parsing the output to determine if the program hash matches and storing the verified build
/// information in a database.
///
/// Arguments:
///
/// * `pool`: `pool` is an Arc of a connection pool to a PostgreSQL database. It is used to interact
/// with the database and perform database operations.
/// * `payload`: The `payload` parameter is of type `SolanaProgramBuildParams`
///
/// Returns:
///
/// The function `verify_build` returns a `Result` with the success case containing a `VerifiedProgram`
/// struct and the error case containing an `ApiError`.
pub async fn verify_build(
    payload: SolanaProgramBuildParams,
    build_id: &str,
) -> Result<VerifiedProgram> {
    tracing::info!("Verifying build..");

    // Original R limit
    let mut original_rlimit = rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // 1 GB memory limit
    let max_ram_usage_bytes: c_ulong = 1024 * 1024 * 1024;
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
    // Run solana-verify command
    let mut cmd = Command::new("solana-verify");
    cmd.arg("verify-from-repo").arg("-um");

    // Add optional arguments
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

    let output = cmd.output().await?;
    let result = String::from_utf8(output.stdout)?;
    if !output.status.success() {
        return Err(ApiError::Build(result));
    }

    let onchain_hash = extract_hash(&result, "On-chain Program Hash:").unwrap_or_default();
    let build_hash =
        extract_hash(&result, "Executable Program Hash from repo:").unwrap_or_default();

    // last line of output has the result
    let last_line = get_last_line(&result).ok_or_else(|| {
        ApiError::Build("Failed to build and get output from program".to_string())
    })?;

    tracing::info!(
        "{} build hash {} On chain hash {}",
        payload.program_id,
        build_hash,
        onchain_hash
    );

    let verified_build = VerifiedProgram {
        id: uuid::Uuid::new_v4().to_string(),
        program_id: payload.program_id,
        is_verified: last_line.contains("Program hash matches"),
        on_chain_hash: onchain_hash,
        executable_hash: build_hash,
        verified_at: chrono::Utc::now().naive_utc(),
        solana_build_id: build_id.to_string(),
    };

    // Reset R limit
    unsafe {
        setrlimit(RLIMIT_AS, &original_rlimit);
    }

    Ok(verified_build)
    // let _ = self.insert_or_update_verified_build(&verified_build).await;
}

pub async fn get_on_chain_hash(program_id: &str) -> Result<String> {
    let mut cmd = Command::new("solana-verify");
    cmd.arg("get-program-hash").arg(program_id);

    let output = cmd
        .output()
        .await
        .map_err(|_| ApiError::Custom("Failed to run process get-program-hash".to_string()))?;

    if !output.status.success() {
        tracing::error!(
            "Failed to get on-chain hash {}",
            String::from_utf8(output.stderr)?
        );
        return Err(ApiError::Custom("Failed to get on-chain hash".to_string()));
    }
    let result = String::from_utf8(output.stdout)?;
    let hash = get_last_line(&result).ok_or_else(|| {
        ApiError::Custom("Failed to build and get output from program".to_string())
    })?;
    Ok(hash)
}
