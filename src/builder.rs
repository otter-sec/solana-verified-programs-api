use tokio::process::Command;

use crate::errors::ApiError;
use crate::models::{SolanaProgramBuildParams, VerifiedProgram};
use crate::Result;

fn get_last_line(output: &str) -> Option<String> {
    output.lines().last().map(ToOwned::to_owned)
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
pub async fn verify_build(payload: SolanaProgramBuildParams) -> Result<VerifiedProgram> {
    tracing::info!("Verifying build..");
    let mut cmd = Command::new("solana-verify");
    cmd.arg("verify-from-repo")
        .arg("-um")
        .arg("--program-id")
        .arg(&payload.program_id)
        .arg(payload.repository);

    if let Some(commit) = payload.commit_hash {
        cmd.arg("--commit-hash").arg(commit);
    }

    if let Some(library_name) = payload.lib_name {
        cmd.arg("--library-name").arg(library_name);
    }

    if let Some(bpf_flag) = payload.bpf_flag {
        if bpf_flag {
            cmd.arg("--bpf");
        }
    }

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
    };
    Ok(verified_build)
    // let _ = self.insert_or_update_verified_build(&verified_build).await;
}
