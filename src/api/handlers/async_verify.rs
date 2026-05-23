use super::verify_helpers::{create_and_insert_build, setup_verification};
use crate::{
    api::responses::{ApiResponse, JobStatus, VerifyResponse},
    build,
    db::{DbClient, NewBuild},
    onchain::is_program_data_missing,
    state::AppState,
    types::{Address, WebhookUrl},
};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use solana_client::nonblocking::rpc_client::RpcClient;
use tracing::{error, info};
use uuid::Uuid;

/// Request body for `POST /verify`. Build params come from the on-chain
/// Otter Verify PDA; only `program_id` (and optionally `webhook_url`) is
/// honoured. Extra fields like `repository` / `commit_hash` are accepted
/// and silently ignored for backward compatibility.
#[derive(Debug, Clone, Deserialize)]
pub struct SolanaProgramBuildParams {
    pub program_id: Address,
    #[serde(default)]
    pub webhook_url: Option<WebhookUrl>,
}

/// Request body for `POST /verify-with-signer`.
#[derive(Debug, Clone, Deserialize)]
pub struct SolanaProgramBuildParamsWithSigner {
    pub program_id: Address,
    pub signer: Address,
    #[serde(default)]
    pub webhook_url: Option<WebhookUrl>,
}

/// Handler for asynchronous program verification
///
/// # Endpoint: POST /verify
pub(crate) async fn process_async_verification(
    State(state): State<AppState>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> (StatusCode, Json<ApiResponse>) {
    info!(
        "Starting async verification for program: {}",
        payload.program_id
    );

    let webhook_url = payload.webhook_url.map(WebhookUrl::into_inner);
    match setup_verification(&state, &payload.program_id, None).await {
        Ok(params) => process_verification(state, params, webhook_url).await,
        Err(error_response) => error_response,
    }
}

/// Handler for asynchronous program verification with a specific signer
///
/// # Endpoint: POST /verify-with-signer
pub(crate) async fn process_async_verification_with_signer(
    State(state): State<AppState>,
    Json(payload): Json<SolanaProgramBuildParamsWithSigner>,
) -> (StatusCode, Json<ApiResponse>) {
    info!(
        "Starting async verification for program {} with signer {}",
        payload.program_id, payload.signer
    );

    let webhook_url = payload.webhook_url.map(WebhookUrl::into_inner);
    match setup_verification(&state, &payload.program_id, Some(payload.signer)).await {
        Ok(params) => process_verification(state, params, webhook_url).await,
        Err(error_response) => error_response,
    }
}

/// Processes the verification request asynchronously
pub async fn process_verification(
    state: AppState,
    payload: NewBuild,
    webhook_url: Option<String>,
) -> (StatusCode, Json<ApiResponse>) {
    // Check for existing verification
    if let Ok(Some(dup)) = state.db.find_duplicate(&payload).await {
        check_program_closed(&state.db, &state.rpc, &payload.program_id).await;
        return (
            StatusCode::OK,
            Json(
                VerifyResponse {
                    status: dup.status,
                    request_id: dup.id.to_string(),
                    message: match dup.status {
                        JobStatus::InProgress => "Build verification already in progress".into(),
                        JobStatus::Completed => "Verification already completed.".into(),
                        JobStatus::Failed => "Build record exists.".into(),
                    },
                }
                .into(),
            ),
        );
    }

    let verification_uuid = match create_and_insert_build(&state.db, &payload).await {
        Ok(uuid) => uuid,
        Err(error_response) => return error_response,
    };

    spawn_verification_task(state, payload, verification_uuid, webhook_url).await;

    (
        StatusCode::OK,
        Json(
            VerifyResponse {
                status: JobStatus::InProgress,
                request_id: verification_uuid.to_string(),
                message: "Build verification started".to_string(),
            }
            .into(),
        ),
    )
}

/// Spawns an asynchronous verification task
async fn spawn_verification_task(
    state: AppState,
    payload: NewBuild,
    uuid: Uuid,
    webhook_url: Option<String>,
) {
    info!("Verification task spawned with UUID: {}", uuid);
    tokio::spawn(async move {
        info!("Spawning verification task with uuid: {}", uuid);
        build::execute(uuid, payload, state, webhook_url).await;
    });
}

/// Checks if the program's buffer account is missing, and if so,
/// marks the program as unverified in the database.
pub async fn check_program_closed(db: &DbClient, rpc: &RpcClient, program_id: &Address) {
    if is_program_data_missing(rpc, &program_id.to_string()).await {
        info!(
            "Program {} buffer missing. Marking as unverified.",
            program_id
        );

        if let Err(e) = db.mark_closed(program_id).await {
            error!(
                "Program {} buffer missing. failed to mark as unverified: {:?}",
                program_id, e
            );
        }
    }
}
