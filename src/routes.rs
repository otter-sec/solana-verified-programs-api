use crate::models::{SolanaProgramBuild, SolanaProgramBuildParams};
use crate::operations::{insert_build, verify_build};
use crate::state::AppState;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde_json::{json, Value};

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
        .route("/verify", post(handle_verify))
        .route("/", get(index))
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
                    "commit": "Commit hash ",
                    "program_id": "Program ID of the program in mainnet",
                    "lib_name": "(Optional) If the repository contains multiple programs, specify the name of the library name of the program to build and verify."
                }
            },
        ]
    }))
}

async fn handle_verify(
    State(app): State<AppState>,
    Json(payload): Json<SolanaProgramBuildParams>,
) -> Json<SolanaProgramBuild> {
    println!("Received payload: {:?}", payload);

    let verify_build_data = SolanaProgramBuild {
        id: uuid::Uuid::new_v4().to_string(),
        repository: payload.repository.clone(),
        commit_hash: payload.commit_hash.clone(),
        program_id: payload.program_id.clone(),
        lib_name: payload.lib_name.clone(),
        created_at: Some(Utc::now().naive_utc()),
    };

    // insert into database
    let insert = insert_build(&verify_build_data, app.db_pool).await;

    match insert {
        Ok(_) => println!("Inserted into database"),
        Err(e) => println!("Error inserting into database: {:?}", e),
    }

    //run task in background
    tokio::spawn(verify_build(payload));

    Json(verify_build_data)
}
