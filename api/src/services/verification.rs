//! Spawning and lifecycle of `solana-verify` builds.

use crate::{
    db::{DbClient, NewBuild},
    errors::{ApiError, Result},
    responses::VerificationWebhookPayload,
    services::logging as logs,
    services::misc::extract_hash_with_prefix,
    state::AppState,
    validation::Address,
};
use chrono::Utc;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command};
use tracing::{error, info};
use uuid::Uuid;

const WEBHOOK_RETRIES: u32 = 3;
const WEBHOOK_RETRY_DELAY: Duration = Duration::from_millis(2000);

/// What [`run_build`] parsed from `solana-verify`'s output. `is_verified` is
/// false if either hash is missing.
#[derive(Debug, Clone)]
pub struct VerifyOutcome {
    pub on_chain_hash: String,
    pub executable_hash: String,
    pub is_verified: bool,
}

/// Runs `solana-verify verify-from-repo` once and parses the output. On
/// failure, writes the logs to disk. Does not update the `builds` row --
/// see [`execute`] for the full lifecycle.
pub async fn run_build(
    build_id: Uuid,
    params: &NewBuild,
    db: &DbClient,
    rpc_url: &str,
) -> Result<VerifyOutcome> {
    let log_id = Uuid::new_v4().to_string();
    info!(
        program = %params.program_id,
        build_id = %build_id,
        "starting solana-verify"
    );

    let mut cmd = build_command(params, rpc_url);
    let mut child = cmd
        .spawn()
        .map_err(|e| ApiError::Build(format!("spawn solana-verify: {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(b"n\n")
            .await
            .map_err(|e| ApiError::Build(format!("write to solana-verify stdin: {e}")))?;
    }
    let output = child
        .wait_with_output()
        .await
        .map_err(|e| ApiError::Build(format!("wait solana-verify: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if let Err(e) = logs::write_logs(&stderr, &stdout, &log_id, rpc_url).await {
            error!("write build logs: {}", e);
        }
        if let Err(e) = db
            .insert_build_log(build_id, &params.program_id, &log_id)
            .await
        {
            error!("insert build log row: {}", e);
        }
        return Err(ApiError::Build(stdout));
    }

    let on_chain_hash =
        extract_hash_with_prefix(&stdout, "On-chain Program Hash:").unwrap_or_default();
    let executable_hash =
        extract_hash_with_prefix(&stdout, "Executable Program Hash from repo:").unwrap_or_default();

    Ok(VerifyOutcome {
        is_verified: !executable_hash.is_empty() && on_chain_hash == executable_hash,
        on_chain_hash,
        executable_hash,
    })
}

// TODO: drive solana-verify in-process if it gains a library API; the
// binary is currently the only way to run `verify-from-repo`, so we shell
// out and parse its stdout via `extract_hash_with_prefix`.
fn build_command(p: &NewBuild, rpc_url: &str) -> Command {
    let mut cmd = Command::new("solana-verify");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("verify-from-repo")
        .arg("--url")
        .arg(rpc_url)
        .arg("--program-id")
        .arg(p.program_id.to_string())
        .arg(&p.repository);
    if let Some(c) = &p.commit_hash {
        cmd.arg("--commit-hash").arg(c);
    }
    if let Some(lib) = &p.lib_name {
        cmd.arg("--library-name").arg(lib);
    }
    if let Some(img) = &p.base_docker_image {
        cmd.arg("--base-image").arg(img);
    }
    if let Some(sbf_args) = &p.cargo_build_sbf_args {
        cmd.arg(format!("--cargo-build-sbf-args={sbf_args}"));
    }
    if let Some(mp) = &p.mount_path {
        cmd.arg("--mount-path").arg(mp);
    }
    if p.bpf_flag {
        cmd.arg("--bpf");
    }
    if let Some(a) = &p.arch {
        cmd.arg("--arch").arg(a);
    }
    if let Some(args) = &p.cargo_args {
        if !args.is_empty() {
            cmd.arg("--").args(args);
        }
    }
    cmd
}

/// End-to-end async build. What gets `tokio::spawn`ed by the verify handlers.
pub async fn execute(
    build_id: Uuid,
    params: NewBuild,
    state: AppState,
    webhook_url: Option<String>,
) {
    let program_id = params.program_id;
    let result = run_build(build_id, &params, &state.db, &state.rpc_url).await;
    let payload = match &result {
        Ok(out) => {
            finalize_completed(&state.db, &state.rpc, build_id, out, &program_id).await;
            VerificationWebhookPayload {
                request_id: build_id.to_string(),
                status: crate::db::JobStatus::Completed,
                is_verified: Some(out.is_verified),
                program_id: Some(program_id),
                on_chain_hash: Some(out.on_chain_hash.clone()),
                executable_hash: Some(out.executable_hash.clone()),
                verified_at: Some(Utc::now().naive_utc()),
                error: None,
            }
        }
        Err(e) => {
            if let Err(err) = state.db.mark_build_failed(build_id, &e.to_string()).await {
                error!("mark failed: {}", err);
            }
            VerificationWebhookPayload {
                request_id: build_id.to_string(),
                status: crate::db::JobStatus::Failed,
                is_verified: None,
                program_id: Some(program_id),
                on_chain_hash: None,
                executable_hash: None,
                verified_at: None,
                error: Some(e.to_string()),
            }
        }
    };

    if let Err(e) = &result {
        // If the upgrade buffer is missing, treat the program as closed.
        if crate::services::onchain::is_program_data_missing(&state.rpc, &program_id.to_string())
            .await
        {
            if let Err(err) = state.db.mark_closed(&program_id).await {
                error!("mark_closed after failed build: {}", err);
            }
        }
        error!("build {} failed: {}", build_id, e);
    }

    if let Some(url) = webhook_url {
        post_webhook(&url, &payload).await;
    }
}

/// Marks the build completed and refreshes `program_state` from chain.
/// Shared by the async [`execute`] and `verify_sync` post-build paths.
pub async fn finalize_completed(
    db: &DbClient,
    rpc: &RpcClient,
    build_id: Uuid,
    outcome: &VerifyOutcome,
    program_id: &Address,
) {
    if let Err(e) = db
        .mark_build_completed(build_id, &outcome.executable_hash)
        .await
    {
        error!("mark completed: {}", e);
    }
    let pid = *program_id.as_pubkey();
    let snapshots = match crate::services::onchain::snapshot_programs(rpc, &[pid]).await {
        Ok(s) => s,
        Err(e) => {
            error!("snapshot {}: {}", program_id, e);
            return;
        }
    };
    if let Some(snap) = snapshots.get(&pid) {
        if let Err(e) = db.upsert_program_state(program_id, snap).await {
            error!("upsert state {}: {}", program_id, e);
        }
    }
}

async fn post_webhook(url: &str, payload: &VerificationWebhookPayload) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("reqwest build: {}", e);
            return;
        }
    };
    for attempt in 0..WEBHOOK_RETRIES {
        match client.post(url).json(payload).send().await {
            Ok(r) if r.status().is_success() => return,
            Ok(r) => error!("webhook {} returned {}", url, r.status()),
            Err(e) => error!("webhook {} send failed: {}", url, e),
        }
        if attempt + 1 < WEBHOOK_RETRIES {
            tokio::time::sleep(WEBHOOK_RETRY_DELAY).await;
        }
    }
}
