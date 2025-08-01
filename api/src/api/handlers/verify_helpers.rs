//! Shared verification helpers and utilities
//! Contains common logic used across different verification endpoints

use crate::{
    db::{
        models::{
            ApiResponse, ErrorResponse, SolanaProgramBuild, SolanaProgramBuildParams, Status,
        },
        DbClient,
    },
    errors::ErrorMessages,
    services::onchain::{self, get_program_authority},
};
use axum::{http::StatusCode, Json};
use tracing::error;

/// Result type for verification setup operations
pub type VerificationSetupResult = Result<VerificationSetup, (StatusCode, Json<ApiResponse>)>;

/// Contains all the setup data needed for verification
pub struct VerificationSetup {
    pub params: SolanaProgramBuildParams,
    pub signer: String,
}

/// Common setup logic for verification endpoints
///
/// Handles:
/// - Getting program authority from on-chain
/// - Fetching verification parameters from PDA
/// - Updating program authority in database
pub async fn setup_verification(
    db: &DbClient,
    program_id: &str,
    specific_signer: Option<String>,
) -> VerificationSetupResult {
    // Get program authority from on-chain
    let (program_authority, is_frozen) = match get_program_authority(program_id).await {
        Ok((authority, frozen)) => (authority, frozen),
        Err(e) => {
            let error_str = e.to_string();
            if error_str.contains("Program appears to be closed") {
                // For closed programs, no authority and frozen=true
                (None, true)
            } else {
                // For other errors, default to no authority and not frozen
                (None, false)
            }
        }
    };

    // Get verification parameters from on-chain PDA
    match onchain::get_otter_verify_params(program_id, specific_signer, program_authority.clone())
        .await
    {
        Ok((params, signer)) => {
            // Update program authority in database
            if let Err(e) = db
                .insert_or_update_program_authority(
                    &params.address,
                    program_authority.as_deref(),
                    is_frozen,
                )
                .await
            {
                error!("Failed to update program authority: {:?}", e);
            }

            Ok(VerificationSetup {
                params: SolanaProgramBuildParams::from(params),
                signer,
            })
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

/// Creates and inserts build parameters into the database
/// Returns the UUID of the created build
pub async fn create_and_insert_build(
    db: &DbClient,
    params: &SolanaProgramBuildParams,
    signer: &str,
) -> Result<String, (StatusCode, Json<ApiResponse>)> {
    let mut build_data = SolanaProgramBuild::from(params);
    build_data.signer = Some(signer.to_string());
    let uuid = build_data.id.clone();

    if let Err(e) = db.insert_build_params(&build_data).await {
        error!("Error inserting build parameters: {:?}", e);
        return Err(create_db_error());
    }

    Ok(uuid)
}
