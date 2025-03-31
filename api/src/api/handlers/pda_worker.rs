use crate::{
    api::handlers::{async_verify::process_verification, is_authorized},
    db::{
        models::{extract_instruction, SolanaProgramBuildParams},
        DbClient,
    },
    services::{get_on_chain_hash, onchain::OtterBuildParams},
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use borsh::BorshDeserialize;
use serde_json::Value;
use tracing::{error, info, warn};

pub(crate) async fn handle_pda_updates_creations(
    State(db): State<DbClient>,
    headers: HeaderMap,
    Json(payload): Json<Vec<Value>>,
) -> (StatusCode, &'static str) {
    info!("Received PDA updates/creations request");

    // Validate authorization
    if !is_authorized(&headers) {
        warn!("Unauthorized unverify attempt");
        return (
            StatusCode::UNAUTHORIZED,
            "Missing or invalid authorization header",
        );
    }

    // Validate payload
    let instruction = match extract_instruction(&payload) {
        Ok(instruction) => instruction,
        Err(status) => return status,
    };

    // Process instructions
    for ix in instruction.instructions {
        let program_id = &ix.accounts[1];
        info!("Processing upgrade instruction for program: {}", program_id);

        let data = ix.data.as_bytes();
        let otter_build_params = match OtterBuildParams::try_from_slice(&data[8..]) {
            Ok(params) => params,
            Err(e) => {
                error!("Failed to deserialize PDA data: {}", e);
                continue;
            }
        };

        if let Err(e) = process_pda_upgrade(&db, program_id, otter_build_params).await {
            error!("Failed to process program upgrade: {}", e);
            continue;
        }
    }

    (StatusCode::OK, "PDA updates/creations request received")
}

async fn process_pda_upgrade(
    db: &DbClient,
    program_id: &str,
    params: OtterBuildParams,
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_hash = db.get_verified_build(program_id, None).await?;

    let onchain_hash = get_on_chain_hash(program_id).await?;

    if onchain_hash != executable_hash.on_chain_hash {
        info!("Program {} has been upgraded, unverifying", program_id);
        db.unverify_program(program_id, &onchain_hash).await?;
        // start new build
        let signer = params.signer.to_string();
        let solana_build_params = SolanaProgramBuildParams::from(params);
        let _ = process_verification(db.clone(), solana_build_params, signer).await;
        info!("Successfully unverified program {}", program_id);
    } else {
        info!("Program {} has not been upgraded", program_id);
    }
    Ok(())
}
