use crate::db::models::{JobStatus, SolanaProgramBuildParams, VerifiedProgram, VerifyResponse};
use crate::db::DbClient;
use crate::errors::ApiError;
use crate::errors::ErrorMessages;
use crate::services::misc::extract_hash;
use crate::{Result, CONFIG};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use uuid::Uuid;

pub async fn check_and_process_verification(
    payload: SolanaProgramBuildParams,
    build_id: &str,
    db: &DbClient,
) -> Result<VerifiedProgram> {
    let random_file_id = Uuid::new_v4().to_string();
    let program_id = payload.program_id.clone();
    let uid = build_id.to_string();

    match verify_build(payload, &uid, &random_file_id).await {
        Ok(res) => {
            let _ = db.insert_or_update_verified_build(&res).await;
            let _ = db
                .update_build_status(&uid, JobStatus::Completed.into())
                .await;
            Ok(res)
        }
        Err(err) => {
            let _ = db.update_build_status(&uid, JobStatus::Failed.into()).await;
            let _ = db
                .insert_logs_info(&random_file_id, &program_id, &uid)
                .await;

            tracing::error!("Error verifying build: {:?}", err);
            tracing::error!("{:?}", ErrorMessages::Unexpected.to_string());
            Err(err)
        }
    }
}

pub async fn check_and_handle_duplicates(
    payload: &SolanaProgramBuildParams,
    db: &DbClient,
) -> Option<VerifyResponse> {
    // Check if the build was already processed
    let is_duplicate = db.check_for_duplicate(payload).await;

    if let Ok(respose) = is_duplicate {
        match respose.status.into() {
            JobStatus::Completed => {
                // Get the verified build from the database
                let verified_build = db.get_verified_build(&respose.program_id).await.unwrap();
                Some(VerifyResponse {
                    status: JobStatus::Completed,
                    request_id: verified_build.solana_build_id,
                    message: "Verification already completed.".to_string(),
                })
            }
            JobStatus::InProgress => {
                // Return ID to user to check status
                Some(VerifyResponse {
                    status: JobStatus::InProgress,
                    request_id: respose.id,
                    message: "Build verification already in progress".to_string(),
                })
            }
            JobStatus::Failed => {
                // Retry build
                tracing::info!("Previous build failed for this program. Initiating new build");
                None
            }
        }
    } else {
        None
    }
}

pub async fn verify_build(
    payload: SolanaProgramBuildParams,
    build_id: &str,
    random_file_id: &str,
) -> Result<VerifiedProgram> {
    tracing::info!("Verifying build..");

    let mut cmd = Command::new("solana-verify");

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd.arg("verify-from-repo");

    cmd.arg("--url").arg(&CONFIG.rpc_url);

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

    let mut child = cmd
        .spawn()
        .expect("Failed to successfully run solana-verify command");

    // Get the stdin handle
    if let Some(mut stdin) = child.stdin.take() {
        // Send 'n' to the process
        stdin.write_all(b"n\n").await?;
    }

    let output = child.wait_with_output().await.map_err(|e| {
        tracing::error!("Error running command: {:?}", e);
        let _ = crate::services::logging::write_logs(
            &e.to_string(),
            "Error running command",
            random_file_id,
        );
        ApiError::Build("Error running command".to_string())
    })?;

    let result = String::from_utf8(output.stdout).unwrap_or_default();
    if !output.status.success() {
        let _ = crate::services::logging::write_logs(
            &String::from_utf8(output.stderr).unwrap_or_default(),
            &result,
            random_file_id,
        );
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

    Ok(verified_build)
}
