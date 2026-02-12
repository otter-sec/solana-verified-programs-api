use crate::{
    db::models::{
        JobStatus, SolanaProgramBuildParams, VerificationWebhookPayload, VerifiedProgram,
        VerifyResponse,
    },
    db::DbClient,
    errors::ApiError,
    services::misc::extract_hash_with_prefix,
    Result, CONFIG,
};
use std::process::Stdio;
use std::time::Duration;
use tokio::{io::AsyncWriteExt, process::Command, time::sleep};
use tracing::{error, info};
use uuid::Uuid;

const MAX_WEBHOOK_RETRIES: u32 = 3;
const WEBHOOK_RETRY_DELAY_MS: u64 = 2000;

/// Processes and verifies a program build
///
/// # Arguments
/// * `payload` - Build parameters for the program
/// * `build_id` - Unique identifier for this build
/// * `db` - Database client for storing results
///
/// # Returns
/// * `Result<VerifiedProgram>` - Verification result if successful
pub async fn process_verification_request(
    payload: SolanaProgramBuildParams,
    build_id: &str,
    db: &DbClient,
) -> Result<VerifiedProgram> {
    let random_file_id = Uuid::new_v4().to_string();
    let program_id = payload.program_id.clone();
    let uid = build_id.to_string();

    match execute_verification(payload, &uid, &random_file_id).await {
        Ok(res) => {
            let insertion_count = db.insert_or_update_verified_build(&res).await?;
            info!("Inserted {} verified builds", insertion_count);
            if let Err(e) = db.update_build_status(&uid, JobStatus::Completed).await {
                error!("Failed to update build status to completed: {:?}", e);
            }
            Ok(res)
        }
        Err(err) => {
            if let Err(e) = db.update_build_status(&uid, JobStatus::Failed).await {
                error!("Failed to update build status to failed: {:?}", e);
            }
            if let Err(e) = db
                .insert_logs_info(&random_file_id, &program_id, &uid)
                .await
            {
                error!("Failed to insert logs info: {:?}", e);
            }
            error!("Build verification failed: {:?}", err);
            Err(err)
        }
    }
}

/// Checks for duplicate verification requests
///
/// # Arguments
/// * `payload` - Build parameters to check
/// * `signer` - Signer of the verification request
/// * `db` - Database client for checking status
///
/// # Returns
/// * `Option<VerifyResponse>` - Response if duplicate found
pub async fn check_and_handle_duplicates(
    payload: &SolanaProgramBuildParams,
    signer: String,
    db: &DbClient,
) -> Option<VerifyResponse> {
    match db.check_for_duplicate(payload, signer).await {
        Ok(response) => match response.status.into() {
            JobStatus::Completed => {
                match db.get_verified_build(&response.program_id, response.signer).await {
                    Ok(verified_build) => Some(VerifyResponse {
                        status: JobStatus::Completed,
                        request_id: verified_build.solana_build_id,
                        message: "Verification already completed.".to_string(),
                    }),
                    Err(err) => {
                        error!("Failed to get verified build: {:?}", err);
                        None
                    }
                }
            }
            JobStatus::InProgress => Some(VerifyResponse {
                status: JobStatus::InProgress,
                request_id: response.id,
                message: "Build verification already in progress".to_string(),
            }),
            JobStatus::Unused => Some(VerifyResponse {
                status: JobStatus::Completed,
                request_id: response.id,
                message: "These params were not used. There might be a PDA associated with this program ID.".to_string(),
            }),
            JobStatus::Failed => {
                info!("Previous build failed, initiating new build");
                None
            }
        },
        Err(_) => None,
    }
}

/// Verifies a program build using solana-verify
///
/// # Arguments
/// * `payload` - Build parameters for verification
/// * `build_id` - Unique identifier for this build
/// * `random_file_id` - Identifier for log files
///
/// # Returns
/// * `Result<VerifiedProgram>` - Verification result if successful
pub async fn execute_verification(
    payload: SolanaProgramBuildParams,
    build_id: &str,
    random_file_id: &str,
) -> Result<VerifiedProgram> {
    info!(
        "Starting build verification for program: {}",
        payload.program_id
    );

    let mut cmd = build_verify_command(&payload)?;

    // Spawn the verification process in a separate task to avoid blocking the async runtime
    let verification_task = async {
        let mut child = cmd.spawn().map_err(|e| {
            error!("Failed to spawn solana-verify command: {}", e);
            ApiError::Build("Failed to start verification process".to_string())
        })?;

        // Handle stdin for the process
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(b"n\n").await.map_err(|e| {
                error!("Failed to write to stdin: {}", e);
                ApiError::Build("Failed to communicate with verification process".to_string())
            })?;
        }

        let output = child.wait_with_output().await.map_err(|e| {
            error!("Failed to get command output: {}", e);
            ApiError::Build("Failed to complete verification process".to_string())
        })?;

        Ok::<std::process::Output, crate::errors::ApiError>(output)
    };

    // Wait for verification to complete (no timeout)
    let output = verification_task.await.map_err(|e| {
        error!(
            "Verification failed for program: {}: {}",
            payload.program_id, e
        );
        e
    })?;

    process_verification_output(output, &payload, build_id, random_file_id).await
}

/// Builds the solana-verify command with appropriate arguments
///
/// # Arguments
/// * `payload` - Build parameters for verification
///
/// # Returns
/// * `Result<Command>` - solana-verify command to execute verification
pub fn build_verify_command(payload: &SolanaProgramBuildParams) -> Result<Command> {
    let mut cmd = Command::new("solana-verify");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("verify-from-repo")
        .arg("--url")
        .arg(&CONFIG.rpc_url)
        .arg("--program-id")
        .arg(&payload.program_id)
        .arg(&payload.repository);

    // Add optional arguments
    if let Some(ref commit) = payload.commit_hash {
        cmd.arg("--commit-hash").arg(commit);
    }
    if let Some(ref lib_name) = payload.lib_name {
        cmd.arg("--library-name").arg(lib_name);
    }
    if let Some(ref base_image) = payload.base_image {
        cmd.arg("--base-image").arg(base_image);
    }
    if let Some(ref mount_path) = payload.mount_path {
        cmd.arg("--mount-path").arg(mount_path);
    }
    if payload.bpf_flag.unwrap_or(false) {
        cmd.arg("--bpf");
    }
    if let Some(ref arch) = payload.arch {
        cmd.arg("--arch").arg(arch);
    }
    if let Some(ref cargo_args) = payload.cargo_args {
        cmd.arg("--").args(cargo_args);
    }

    info!("Prepared command: {:?}", cmd);
    Ok(cmd)
}

/// Processes the output from the verification command
///
/// # Arguments
/// * `output` - Output from the verification command
/// * `payload` - Build parameters for verification
/// * `build_id` - Unique identifier for this build
/// * `random_file_id` - Identifier for log files
///
/// # Returns
/// * `Result<VerifiedProgram>` - Verification result if successful
async fn process_verification_output(
    output: std::process::Output,
    payload: &SolanaProgramBuildParams,
    build_id: &str,
    random_file_id: &str,
) -> Result<VerifiedProgram> {
    let stdout = String::from_utf8(output.stdout).unwrap_or_default();

    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr).unwrap_or_default();
        if let Err(e) = crate::services::logging::write_logs(&stderr, &stdout, random_file_id).await
        {
            error!("Failed to write logs: {:?}", e);
        }
        return Err(ApiError::Build(stdout));
    }

    let onchain_hash =
        extract_hash_with_prefix(&stdout, "On-chain Program Hash:").unwrap_or_default();
    let build_hash =
        extract_hash_with_prefix(&stdout, "Executable Program Hash from repo:").unwrap_or_default();

    info!(
        "Verification complete - Program: {}, Build hash: {}, On-chain hash: {}",
        payload.program_id, build_hash, onchain_hash
    );

    Ok(VerifiedProgram {
        id: Uuid::new_v4().to_string(),
        program_id: payload.program_id.clone(),
        is_verified: onchain_hash == build_hash,
        on_chain_hash: onchain_hash,
        executable_hash: build_hash,
        verified_at: chrono::Utc::now().naive_utc(),
        solana_build_id: build_id.to_string(),
    })
}

/// Notifies the webhook about the verification result
///
/// # Arguments
/// * `webhook_url` - URL to post the verification result
/// * `result` - Result of the verification process
/// * `request_id` - Unique identifier for the verification request
///
/// constructs the payload and spawns a task to post the payload to the webhook URL
/// 
pub fn notify_webhook(
    webhook_url: String,
    result: std::result::Result<VerifiedProgram, ApiError>,
    request_id: String,
) {
    let payload = match &result {
        Ok(v) => VerificationWebhookPayload {
            request_id: request_id.clone(),
            status: "completed".to_string(),
            is_verified: Some(v.is_verified),
            program_id: Some(v.program_id.clone()),
            on_chain_hash: Some(v.on_chain_hash.clone()),
            executable_hash: Some(v.executable_hash.clone()),
            verified_at: Some(v.verified_at),
            error: None,
        },
        Err(e) => VerificationWebhookPayload {
            request_id,
            status: "failed".to_string(),
            is_verified: None,
            program_id: None,
            on_chain_hash: None,
            executable_hash: None,
            verified_at: None,
            error: Some(e.to_string()),
        },
    };
    tokio::spawn(async move {
        if let Err(e) = post_webhook(&webhook_url, &payload).await {
            error!("Webhook failed to post payload to {}: {:?}", webhook_url, e);
        }
    });
}

/// POSTs the verification result payload to the webhook URL
///
/// # Arguments
/// * `url` - URL to post the verification result
/// * `payload` - Verification result payload
///
/// # Returns
/// * `Result<(), Box<dyn std::error::Error + Send + Sync>>` - Result of the POST request
async fn post_webhook(
    url: &str,
    payload: &VerificationWebhookPayload,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let mut last_error = None;
    for attempt in 0..MAX_WEBHOOK_RETRIES {
        match client.post(url).json(payload).send().await {
            Ok(res) => match res.error_for_status() {
                Ok(_) => return Ok(()),
                Err(e) => last_error = Some(e.into()),
            },
            Err(e) => last_error = Some(e.into()),
        }
        if attempt < MAX_WEBHOOK_RETRIES - 1 {
            sleep(Duration::from_millis(WEBHOOK_RETRY_DELAY_MS)).await;
        }
    }
    Err(last_error
        .unwrap_or_else(|| Box::from(std::io::Error::other("webhook post failed after retries"))))
}
