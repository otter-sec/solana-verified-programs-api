use super::async_verify::SolanaProgramBuildParams;
use super::verify_helpers::{create_and_insert_build, create_internal_error, setup_verification};
use crate::{
    db::NewBuild,
    responses::{ApiResponse, StatusResponse},
    services::build_repository_url,
    services::verification as build,
    state::AppState,
};
use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use tracing::{error, info};

/// `POST /verify_sync` -- verify synchronously and respond with the result.
pub(crate) async fn process_sync_verification(
    State(state): State<AppState>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> (StatusCode, Json<ApiResponse>) {
    info!(
        "Starting synchronous verification for program: {}",
        payload.program_id
    );

    match setup_verification(&state, &payload.program_id, None).await {
        Ok(setup) => process_verification_sync(state, setup.params, setup.signer).await,
        Err(error_response) => error_response,
    }
}

/// Processes the verification request synchronously
async fn process_verification_sync(
    state: AppState,
    mut params: NewBuild,
    signer: crate::validation::Address,
) -> (StatusCode, Json<ApiResponse>) {
    // The dedupe predicate matches every NewBuild field including `signer`;
    // populate it before the lookup or we'd never match.
    params.signer = Some(signer);

    if let Ok(Some(dup)) = state.db.find_duplicate(&params).await {
        return (
            StatusCode::OK,
            Json(
                crate::responses::VerifyResponse {
                    status: dup.status,
                    request_id: dup.id.to_string(),
                    message: match dup.status {
                        crate::responses::JobStatus::InProgress => {
                            "Build verification already in progress".into()
                        }
                        crate::responses::JobStatus::Completed => {
                            "Verification already completed.".into()
                        }
                        crate::responses::JobStatus::Failed => "Build record exists.".into(),
                    },
                }
                .into(),
            ),
        );
    }

    let uuid = match create_and_insert_build(&state.db, &params).await {
        Ok(uuid) => uuid,
        Err(error_response) => return error_response,
    };

    match build::run_build(uuid, &params, &state.db, &state.rpc_url).await {
        Ok(outcome) => {
            build::finalize_completed(&state.db, &state.rpc, uuid, &outcome, &params.program_id)
                .await;
            info!(
                "Verification completed for program: {} (verified: {})",
                params.program_id, outcome.is_verified
            );

            (
                StatusCode::OK,
                Json(
                    StatusResponse {
                        is_verified: outcome.is_verified,
                        message: if outcome.is_verified {
                            "On chain program verified"
                        } else {
                            "On chain program not verified"
                        }
                        .to_string(),
                        on_chain_hash: outcome.on_chain_hash,
                        executable_hash: outcome.executable_hash,
                        last_verified_at: Some(Utc::now().naive_utc()),
                        repo_url: build_repository_url(
                            &params.repository,
                            params.commit_hash.as_deref(),
                        ),
                        commit: params.commit_hash.clone().unwrap_or_default(),
                    }
                    .into(),
                ),
            )
        }
        Err(err) => {
            error!("Verification failed: {:?}", err);
            if let Err(e) = state.db.mark_build_failed(uuid, &err.to_string()).await {
                error!("Failed to mark build as failed: {:?}", e);
            }
            create_internal_error()
        }
    }
}
