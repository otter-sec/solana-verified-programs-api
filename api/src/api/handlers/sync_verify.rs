use crate::db::models::{
    ApiResponse, ErrorResponse, SolanaProgramBuild, SolanaProgramBuildParams,
    SolanaProgramBuildParamsWithSigner, Status, StatusResponse, DEFAULT_SIGNER,
};
use crate::db::DbClient;
use crate::errors::ErrorMessages;
use crate::services::verification::{check_and_handle_duplicates, check_and_process_verification};
use axum::{extract::State, http::StatusCode, Json};

pub(crate) async fn process_sync_verification(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> (StatusCode, Json<ApiResponse>) {
    let verify_build_data = SolanaProgramBuild::from(&payload);
    let uuid = verify_build_data.id.clone();

    // First check if the program is already verified
    let is_dublicate = check_and_handle_duplicates(
        &SolanaProgramBuildParamsWithSigner {
            params: payload.clone(),
            signer: DEFAULT_SIGNER.to_string(),
        },
        &db,
    )
    .await;

    if let Some(response) = is_dublicate {
        return (StatusCode::OK, Json(response.into()));
    }
    // insert into database
    if let Err(e) = db.insert_build_params(&verify_build_data).await {
        tracing::error!("Error inserting into database: {:?}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                ErrorResponse {
                    status: Status::Error,
                    error: ErrorMessages::DB.to_string(),
                }
                .into(),
            ),
        );
    }

    // run task and wait for it to finish
    match check_and_process_verification(payload, &uuid, &db).await {
        Ok(res) => (
            StatusCode::OK,
            Json(
                StatusResponse {
                    is_verified: res.is_verified,
                    message: if res.is_verified {
                        "On chain program verified".to_string()
                    } else {
                        "On chain program not verified".to_string()
                    },
                    on_chain_hash: res.on_chain_hash,
                    executable_hash: res.executable_hash,
                    last_verified_at: Some(res.verified_at),
                    repo_url: verify_build_data
                        .commit_hash
                        .as_ref()
                        .map_or(verify_build_data.repository.clone(), |hash| {
                            format!("{}/tree/{}", verify_build_data.repository, hash)
                        }),
                    commit: verify_build_data.commit_hash.unwrap_or_default(),
                }
                .into(),
            ),
        ),
        Err(err) => {
            tracing::error!("Error verifying build: {:?}", err);
            tracing::error!("{:?}", ErrorMessages::Unexpected.to_string());
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
    }
}
