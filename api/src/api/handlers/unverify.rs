use crate::db::models::UpgradeProgramInstruction;
use crate::db::DbClient;
use crate::services::get_on_chain_hash;
use crate::CONFIG;
use axum::http::HeaderMap;
use axum::{extract::State, http::StatusCode, Json};
use serde_json::Value;

pub async fn handle_unverify(
    State(db): State<DbClient>,
    headers: HeaderMap,
    Json(payload): Json<Vec<Value>>, // Accept the structured payload
) -> (StatusCode, &'static str) {
    // Check if the request is coming from the correct source
    let auth_header = headers
        .get("AUTHORIZATION")
        .and_then(|value| value.to_str().ok());

    match auth_header {
        Some(header_value) => {
            if header_value == CONFIG.auth_secret {
                // get the first element of the payload
                let instruction = serde_json::from_value(payload[0].clone());
                match instruction {
                    Ok(instruction) => {
                        let instruction: UpgradeProgramInstruction = instruction;
                        for ix in instruction.instructions {
                            if ix.data == "5Sxr3" {
                                let program_id = ix.accounts[1].clone();
                                // get on-chain program hash
                                let onchain_hash = get_on_chain_hash(&program_id).await;
                                let executable_hash = db.get_verified_build(&program_id).await;

                                if let Ok(executable_hash) = executable_hash {
                                    if let Ok(onchain_hash) = onchain_hash {
                                        if onchain_hash != executable_hash.on_chain_hash {
                                            tracing::info!(
                                                "Program ID: {} has been upgraded",
                                                program_id
                                            );
                                            let _ = db
                                                .unverify_program(&program_id, &onchain_hash)
                                                .await;
                                            tracing::info!(
                                                "Program ID: {} has been unverified",
                                                program_id
                                            );
                                        } else {
                                            tracing::info!(
                                                "Program ID: {} is not upgraded",
                                                program_id
                                            );
                                        }
                                    }
                                } else {
                                    tracing::info!("Program ID: {} is not verified", program_id);
                                }
                            }
                        }
                        (StatusCode::OK, "Unverify request received")
                    }
                    Err(e) => {
                        tracing::error!("Error: {:?}", e);
                        (StatusCode::BAD_REQUEST, "Invalid payload")
                    }
                }
            } else {
                (StatusCode::UNAUTHORIZED, "Invalid authorization header")
            }
        }
        None => (
            StatusCode::UNAUTHORIZED,
            "Missing or invalid authorization header",
        ),
    }
}
