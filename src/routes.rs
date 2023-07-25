use crate::builder::verify_build;
use crate::db::DbClient;
use crate::models::{
    ApiResponse, ErrorResponse, SolanaProgramBuild, SolanaProgramBuildParams, Status,
    StatusResponse, VerificationStatusParams, VerifyResponse,
};
use axum::extract::Path;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::OnceLock;

pub fn create_router(db: DbClient) -> Router {
    Router::new()
        .route("/", get(|| async { index() }))
        .route("/verify", post(verify_async))
        .route("/verify_sync", post(verify_sync))
        .route("/status/:address", get(verify_status))
        .with_state(db)
}

static INDEX_JSON: OnceLock<Value> = OnceLock::new();

fn index() -> Json<Value> {
    let value = INDEX_JSON.get_or_init(||
        json!({
            "endpoints": [
                {
                    "path": "/verify",
                    "method": "POST",
                    "description": "Verify a program",
                    "params" : {
                        "repo": "Git repository URL",
                        "commit": "(Optional) Commit hash of the repository. If not specified, the latest commit will be used.",
                        "program_id": "Program ID of the program in mainnet",
                        "lib_name": "(Optional) If the repository contains multiple programs, specify the name of the library name of the program to build and verify.",
                        "bpf_flag": "(Optional)  If the program requires cargo build-bpf (instead of cargo build-sbf), as for an Anchor program, set this flag."
                    }
                },
            ]
        })
    );
    Json(value.clone())
}

// Route handler for POST /verify which creates a new process to verify the program
async fn verify_async(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> Json<ApiResponse> {
    let verify_build_data = SolanaProgramBuild {
        id: uuid::Uuid::new_v4().to_string(),
        repository: payload.repository.clone(),
        commit_hash: payload.commit_hash.clone(),
        program_id: payload.program_id.clone(),
        lib_name: payload.lib_name.clone(),
        bpf_flag: payload.bpf_flag.unwrap_or(false),
        created_at: Utc::now().naive_utc(),
    };

    // First check if the program is already verified
    let is_exists = db
        .check_is_build_params_exists_already(&payload)
        .await
        .unwrap_or(false);

    if is_exists {
        return Json(ApiResponse::Error(ErrorResponse {
            status: Status::Error,
            error: "We have already processed this request".to_string(),
        }));
    }

    // insert into database
    if let Err(e) = db.insert_or_update_build(&verify_build_data).await {
        tracing::error!("Error inserting into database: {:?}", e);
        return Json(
            ErrorResponse {
                status: Status::Error,
                error: "unexpected error occurred".to_string(),
            }
            .into(),
        );
    }

    tracing::info!("Inserted into database");
    //run task in background
    tokio::spawn(async move {
        match verify_build(payload).await {
            Ok(res) => {
                let _ = db.insert_or_update_verified_build(&res).await;
            }
            Err(err) => {
                tracing::error!("Error verifying build: {:?}", err);
            }
        }
    });

    Json(
        VerifyResponse {
            status: Status::Success,
            message: "Build verification started".to_string(),
        }
        .into(),
    )
}

async fn verify_sync(
    State(db): State<DbClient>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> Json<ApiResponse> {
    let verify_build_data = SolanaProgramBuild {
        id: uuid::Uuid::new_v4().to_string(),
        repository: payload.repository.clone(),
        commit_hash: payload.commit_hash.clone(),
        program_id: payload.program_id.clone(),
        lib_name: payload.lib_name.clone(),
        bpf_flag: payload.bpf_flag.unwrap_or(false),
        created_at: Utc::now().naive_utc(),
    };

    // First check if the program is already verified
    let is_exists = db
        .check_is_build_params_exists_already(&payload)
        .await
        .unwrap_or(false);

    if is_exists {
        return Json(ApiResponse::Error(ErrorResponse {
            status: Status::Error,
            error: "We have already processed this request".to_string(),
        }));
    }

    // insert into database
    if let Err(e) = db.insert_or_update_build(&verify_build_data).await {
        tracing::error!("Error inserting into database: {:?}", e);
        return Json(
            ErrorResponse {
                status: Status::Error,
                error: "unexpected error occurred".to_string(),
            }
            .into(),
        );
    }

    tracing::info!("Inserted into database");

    // run task and wait for it to finish
    match verify_build(payload).await {
        Ok(res) => {
            let _ = db.insert_or_update_verified_build(&res).await;
            Json(
                StatusResponse {
                    is_verified: res.is_verified,
                    message: if res.is_verified {
                        "On chain program verified".to_string()
                    } else {
                        "On chain program not verified".to_string()
                    },
                }
                .into(),
            )
        }
        Err(err) => {
            tracing::error!("Error verifying build: {:?}", err);
            Json(
                ErrorResponse {
                    status: Status::Error,
                    error: "unexpected error occurred".to_string(),
                }
                .into(),
            )
        }
    }
}

//  Route handler for GET /status/:address which checks if the program is verified or not
async fn verify_status(
    State(db): State<DbClient>,
    Path(VerificationStatusParams { address }): Path<VerificationStatusParams>,
) -> Json<ApiResponse> {
    match db.check_is_program_verified_within_24hrs(address).await {
        Ok(result) => Json(
            StatusResponse {
                is_verified: result,
                message: if result {
                    "On chain program verified".to_string()
                } else {
                    "On chain program not verified".to_string()
                },
            }
            .into(),
        ),
        Err(err) => {
            tracing::error!("Error getting data from database: {}", err);
            Json(
                ErrorResponse {
                    status: Status::Error,
                    error: "unexpected error occurred".to_string(),
                }
                .into(),
            )
        }
    }
}
