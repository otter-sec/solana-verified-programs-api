use std::str::FromStr;

use crate::{
    api::handlers::{async_verify::process_verification, is_authorized},
    db::{
        models::{parse_helius_transaction, SolanaProgramBuildParams},
        DbClient,
    },
    services::{get_on_chain_hash, onchain::OtterBuildParams},
    CONFIG,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use borsh::BorshDeserialize;
use serde_json::Value;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tracing::{error, info, warn};

pub(crate) async fn handle_pda_updates_creations(
    State(db): State<DbClient>,
    headers: HeaderMap,
    Json(payload): Json<Vec<Value>>,
) -> (StatusCode, &'static str) {
    info!("Received PDA updates/creation event");

    // Validate authorization
    if !is_authorized(&headers) {
        warn!("Unauthorized unverify attempt");
        return (
            StatusCode::UNAUTHORIZED,
            "Missing or invalid authorization header",
        );
    }

    // Validate payload
    let helius_parsed_transaction = match parse_helius_transaction(&payload) {
        Ok(parsed_transaction) => parsed_transaction,
        Err(status) => return status,
    };

    // Process instructions
    for ix in helius_parsed_transaction.instructions {
        // Only process PDA updates/creations
        if ix.programId != "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC" {
            continue;
        }
        let pda_account = &ix.accounts[0];
        let program_id = &ix.accounts[2];

        let _ = process_otter_verify_instruction(&db, program_id, pda_account).await;
    }

    (StatusCode::OK, "PDA updates/creations request received")
}

async fn process_otter_verify_instruction(
    db: &DbClient,
    program_id: &str,
    pda_account: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_hash = match db.get_verified_build(program_id, None).await {
        Ok(data) => data.on_chain_hash,
        Err(_) => String::default(),
    };

    let onchain_hash = get_on_chain_hash(program_id).await?;

    if onchain_hash != executable_hash {
        db.unverify_program(program_id, &onchain_hash).await?;
        // start new build
        let rpc_client = RpcClient::new(CONFIG.rpc_url.clone());
        let pda_account_pubkey = Pubkey::from_str(pda_account)?;
        let params = rpc_client.get_account_data(&pda_account_pubkey).await?;
        let otter_build_params = match OtterBuildParams::try_from_slice(&params[8..]) {
            Ok(params) => params,
            Err(e) => {
                error!("Failed to deserialize PDA data: {}", e);
                return Err(e.into());
            }
        };
        let signer = otter_build_params.signer.to_string();
        let solana_build_params = SolanaProgramBuildParams::from(otter_build_params);
        let _ = process_verification(db.clone(), solana_build_params, signer).await;
        info!("Successfully unverified program {}", program_id);
    } else {
        info!("Program {} has not been upgraded", program_id);
    }
    Ok(())
}
