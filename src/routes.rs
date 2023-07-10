use crate::models::{SolanaProgramBuild, SolanaProgramBuildParams, VerificationStatusParams};
use crate::operations::{check_is_program_verified, insert_build, verify_build};
use crate::state::AppState;
use axum::extract::Path;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde_json::{json, Value};

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/verify", post(handle_verify))
        .route("/verify_sync", post(handle_verify_sync))
        .route("/status/:address", get(handle_verify_status))
        .with_state(app_state)
}

async fn index() -> Json<Value> {
    Json(json!({
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
    }))
}

// Route handler for POST /verify which creates a new process to verify the program
async fn handle_verify(
    State(app): State<AppState>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> Json<Value> {
    let verify_build_data = SolanaProgramBuild {
        id: uuid::Uuid::new_v4().to_string(),
        repository: payload.repository.clone(),
        commit_hash: payload.commit_hash.clone(),
        program_id: payload.program_id.clone(),
        lib_name: payload.lib_name.clone(),
        bpf_flag: payload.bpf_flag.unwrap_or(false),
        created_at: Utc::now().naive_utc(),
    };

    // insert into database
    let insert = insert_build(&verify_build_data, app.db_pool.clone()).await;

    match insert {
        Ok(_) => {
            tracing::info!("Inserted into database");
            //run task in background
            tokio::spawn(async move {
                let _ = verify_build(app.db_pool.clone(), payload).await;
            });

            Json(json!(
                {
                    "success": true,
                    "message": "Build verification started",
                }
            ))
        }
        Err(e) => {
            tracing::error!("Error inserting into database: {:?}", e);
            Json(json!(
                {
                    "success": false,
                    "error": format!("unexpected database error occurred {:?}", e)
                }
            ))
        }
    }
}

async fn handle_verify_sync(
    State(app): State<AppState>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> Json<Value> {
    let verify_build_data = SolanaProgramBuild {
        id: uuid::Uuid::new_v4().to_string(),
        repository: payload.repository.clone(),
        commit_hash: payload.commit_hash.clone(),
        program_id: payload.program_id.clone(),
        lib_name: payload.lib_name.clone(),
        bpf_flag: payload.bpf_flag.unwrap_or(false),
        created_at: Utc::now().naive_utc(),
    };

    // insert into database
    let insert = insert_build(&verify_build_data, app.db_pool.clone()).await;

    match insert {
        Ok(_) => {
            tracing::info!("Inserted into database");
            //run task in background
            // let result = verify_build(app.db_pool.clone(), payload).await;
            let handle =
                tokio::task::spawn_blocking(move || verify_build(app.db_pool.clone(), payload));

            let task = handle.await;

            if let Ok(res) = task {
                match res.await {
                    Ok(verified_program) => {
                        tracing::info!("Build verification completed");
                        Json(json!(
                            {
                                "success": true,
                                "message": "Build verification completed",
                                "on_chain_hash": verified_program.on_chain_hash,
                                "executable_hash": verified_program.executable_hash,
                            }
                        ))
                    }
                    Err(e) => {
                        tracing::error!("Error verifying build: {:?}", e);
                        Json(json!(
                            {
                                "success": false,
                                "error": format!("unexpected error occurred {:?}", e)
                            }
                        ))
                    }
                }
            } else {
                let err_info = format!("unexpected error occurred {:?}", task.err());
                tracing::error!(err_info);
                Json(json!(
                    {
                        "success": false,
                        "error": err_info,
                    }
                ))
            }
        }
        Err(e) => {
            tracing::error!("Error inserting into database: {:?}", e);
            Json(json!(
                {
                    "success": false,
                    "error": format!("unexpected error occurred")
                }
            ))
        }
    }
}
async fn handle_verify_status(
    State(app): State<AppState>,
    Path(VerificationStatusParams { address }): Path<VerificationStatusParams>,
) -> Json<Value> {
    let result = check_is_program_verified(address, app.db_pool.clone()).await;

    if let Ok(result) = result {
        return Json(json!(
            {
                "success": result,
                "message": if result {
                    "On chain program verified"
                } else {
                    "On chain program not verified"
                }
            }
        ));
    }
    Json(json!(
        {
            "status": false,
            "error": "Unexpected error occurred",
        }
    ))
}
