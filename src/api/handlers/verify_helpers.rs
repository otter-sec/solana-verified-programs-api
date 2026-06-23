//! Shared verification helpers and utilities
//! Contains common logic used across different verification endpoints

#![allow(clippy::result_large_err)]

use crate::{
    api::responses::{ApiResponse, ErrorResponse, Status},
    db::{DbClient, NewBuild},
    errors::ErrorMessages,
    onchain::{self, ProgramOnchainState},
    state::AppState,
    types::Address,
};
use axum::{http::StatusCode, Json};
use tracing::error;
use uuid::Uuid;

/// Result type for verification setup operations
pub type VerificationSetupResult = Result<NewBuild, (StatusCode, Json<ApiResponse>)>;

/// Fetches the on-chain program state + Otter Verify PDA params, refreshes
/// the cached `program_state` row, and returns the `NewBuild` to insert.
pub async fn setup_verification(
    app: &AppState,
    program_id: &Address,
    specific_signer: Option<Address>,
) -> VerificationSetupResult {
    let state = onchain::get_program_state(&app.rpc, program_id.as_pubkey())
        .await
        .unwrap_or(ProgramOnchainState {
            authority: None,
            is_frozen: false,
            is_closed: false,
            executable_hash: None,
        });

    match onchain::get_otter_verify_params(
        &app.rpc,
        &program_id.to_string(),
        specific_signer.map(|s| s.to_string()),
        state.authority.clone(),
    )
    .await
    {
        Ok((params, _signer)) => {
            if let Err(e) = app
                .db
                .upsert_program_state(&Address(params.address), &state)
                .await
            {
                error!("Failed to update program state: {:?}", e);
            }

            Ok(NewBuild::from(&params))
        }
        Err(err) => {
            error!(
                "Unable to find on-chain PDA for program {}: {:?}",
                program_id, err
            );
            Err(create_not_found_error())
        }
    }
}

/// Creates a standardized "not found" error response for missing PDAs
pub fn create_not_found_error() -> (StatusCode, Json<ApiResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(
            ErrorResponse {
                status: Status::Error,
                error: ErrorMessages::NoPDA.to_string(),
            }
            .into(),
        ),
    )
}

/// Creates a standardized database error response
pub fn create_db_error() -> (StatusCode, Json<ApiResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(
            ErrorResponse {
                status: Status::Error,
                error: ErrorMessages::DB.to_string(),
            }
            .into(),
        ),
    )
}

/// Creates a standardized internal server error response
pub fn create_internal_error() -> (StatusCode, Json<ApiResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(
            ErrorResponse {
                status: Status::Error,
                error: ErrorMessages::Unexpected.to_string(),
            }
            .into(),
        ),
    )
}

/// Inserts the build row and returns its UUID. Caller must set
/// `params.signer` first.
pub async fn create_and_insert_build(
    db: &DbClient,
    params: &NewBuild,
) -> Result<Uuid, (StatusCode, Json<ApiResponse>)> {
    db.insert_build(params).await.map_err(|e| {
        error!("Error inserting build parameters: {:?}", e);
        create_db_error()
    })
}
