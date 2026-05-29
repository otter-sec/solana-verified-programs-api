use std::str::FromStr;

use crate::{
    api::handlers::{async_verify::process_verification, is_authorized, parse_helius_transaction},
    db::NewBuild,
    services::onchain::{get_on_chain_hash, OtterBuildParams, OTTER_VERIFY_PROGRAM_ID},
    state::AppState,
    validation::Address,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use borsh::BorshDeserialize;
use serde_json::Value;
use solana_pubkey::Pubkey;
use tracing::{error, info, warn};

pub(crate) async fn handle_pda_updates_creations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Vec<Value>>,
) -> (StatusCode, &'static str) {
    info!("Received PDA updates/creation event");

    if !is_authorized(&headers, &state.auth_secret) {
        warn!("Unauthorized PDA webhook attempt");
        return (
            StatusCode::UNAUTHORIZED,
            "Missing or invalid authorization header",
        );
    }

    let helius_parsed_transaction = match parse_helius_transaction(&payload) {
        Ok(parsed_transaction) => parsed_transaction,
        Err(status) => return status,
    };

    // Process instructions in the background -- Helius needs a fast 200.
    tokio::spawn(async move {
        let otter_program_id = OTTER_VERIFY_PROGRAM_ID.to_string();
        for ix in helius_parsed_transaction.instructions {
            if ix.program_id != otter_program_id {
                continue;
            }
            let Some(pda_str) = ix.accounts.first() else {
                warn!("PDA instruction missing accounts");
                continue;
            };
            let Some(program_str) = ix.accounts.get(2) else {
                warn!("PDA instruction missing program account");
                continue;
            };
            let Ok(pda_account) = Pubkey::from_str(pda_str) else {
                warn!("Invalid PDA account in instruction");
                continue;
            };
            let Ok(program_id) = Address::from_str(program_str) else {
                warn!("Invalid program id in PDA instruction");
                continue;
            };

            if let Err(e) =
                process_otter_verify_instruction(state.clone(), &program_id, &pda_account).await
            {
                error!(
                    "Failed to process PDA instruction for {}: {}",
                    program_id, e
                );
            }
        }
    });

    (StatusCode::OK, "PDA updates/creations request received")
}

async fn process_otter_verify_instruction(
    state: AppState,
    program_id: &Address,
    pda_account: &Pubkey,
) -> Result<(), Box<dyn std::error::Error>> {
    let cached_hash = state
        .db
        .cached_on_chain_hash(program_id)
        .await
        .ok()
        .flatten();

    let onchain_hash = match get_on_chain_hash(&state.rpc, program_id).await {
        Ok(hash) => hash,
        Err(e) if e.to_string().contains("Program appears to be closed") => {
            state.db.mark_closed(program_id).await?;
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    // No cached hash (never seen) counts as changed: a PDA event means the
    // program is being verified, so go ahead and (re)build it.
    if cached_hash.as_deref() != Some(onchain_hash.as_str()) {
        state.db.unverify_program(program_id, &onchain_hash).await?;
        // start new build
        let params = state
            .rpc
            .get_account_data(pda_account)
            .await
            .map_err(|e| crate::errors::ApiError::Custom(format!("RPC error: {e}")))?;
        let body = params.get(8..).ok_or_else(|| {
            crate::errors::ApiError::Custom("PDA account data is too short".to_string())
        })?;
        let otter_build_params = match OtterBuildParams::try_from_slice(body) {
            Ok(params) => params,
            Err(e) => {
                error!("Failed to deserialize PDA data: {}", e);
                return Err(e.into());
            }
        };
        let new_build = NewBuild::from(&otter_build_params);
        let _ = process_verification(state, new_build, None).await;
        info!("Re-verification triggered for program {}", program_id);
    } else {
        info!("Program {} has not been upgraded", program_id);
    }
    Ok(())
}
