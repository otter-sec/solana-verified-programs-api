use crate::{
    api::handlers::{is_authorized, parse_helius_transaction},
    db::DbClient,
    services::onchain::get_on_chain_hash,
    state::AppState,
    validation::Address,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::Value;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::str::FromStr;
use tracing::{error, info, warn};

/// Constant for the upgrade instruction data identifier
const UPGRADE_INSTRUCTION_DATA: &str = "5Sxr3";

/// `POST /unverify` -- Helius webhook for upgrade instructions. We respond
/// 200 immediately and process in a background task.
pub async fn handle_unverify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Vec<Value>>,
) -> (StatusCode, &'static str) {
    info!("Received unverify request");

    if !is_authorized(&headers, &state.auth_secret) {
        warn!("Unauthorized unverify attempt");
        return (
            StatusCode::UNAUTHORIZED,
            "Missing or invalid authorization header",
        );
    }

    let helius_parsed_transaction = match parse_helius_transaction(&payload) {
        Ok(parsed_transaction) => parsed_transaction,
        Err(status) => return status,
    };

    let AppState { db, rpc, .. } = state;
    tokio::spawn(async move {
        for ix in helius_parsed_transaction.instructions {
            if ix.data == UPGRADE_INSTRUCTION_DATA {
                let Some(addr) = ix.accounts.get(1) else {
                    warn!("Upgrade instruction missing program account");
                    continue;
                };
                let Ok(program_id) = Address::from_str(addr) else {
                    warn!("Invalid program id in unverify instruction");
                    continue;
                };
                info!("Processing upgrade instruction for program: {}", program_id);

                if let Err(e) = process_program_upgrade(&db, &rpc, &program_id).await {
                    error!("Failed to process program upgrade: {}", e);
                    continue;
                }
            }
        }
    });

    (StatusCode::OK, "Unverify request received")
}

/// Processes a program upgrade by checking and updating verification status
async fn process_program_upgrade(
    db: &DbClient,
    rpc: &RpcClient,
    program_id: &Address,
) -> Result<(), Box<dyn std::error::Error>> {
    let cached_hash = db.cached_on_chain_hash(program_id).await?;

    let onchain_hash = match get_on_chain_hash(rpc, program_id).await {
        Ok(hash) => hash,
        Err(e) if e.to_string().contains("Program appears to be closed") => {
            db.mark_closed(program_id).await?;
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    if onchain_hash != cached_hash {
        info!("Program {} has been upgraded, unverifying", program_id);
        db.unverify_program(program_id, &onchain_hash).await?;
        info!("Successfully unverified program {}", program_id);
    } else {
        info!("Program {} has not been upgraded", program_id);
    }

    Ok(())
}
