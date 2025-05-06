use crate::{
    api::handlers::is_authorized, db::{models::parse_helius_transaction, DbClient}, logging::log_to_file, services::get_on_chain_hash
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::{to_value, Value};
use tracing::{error, info, warn};

/// Constant for the upgrade instruction data identifier
const UPGRADE_INSTRUCTION_DATA: &str = "5Sxr3";

/// Handler for unverifying a program after an upgrade
///
/// # Endpoint: POST /unverify
///
/// # Arguments
/// * `db` - Database client from application state
/// * `headers` - Request headers containing authorization
/// * `payload` - Vector of instruction data
///
/// # Returns
/// * `(StatusCode, &'static str)` - Status code and response message
///
/// # Security
/// Requires valid authorization header matching CONFIG.auth_secret
pub async fn handle_unverify(
    State(db): State<DbClient>,
    headers: HeaderMap,
    Json(payload): Json<Vec<Value>>,
) -> (StatusCode, &'static str) {
    let payload_value = to_value(&payload).ok();
    log_to_file("POST", "/unverify", payload_value.as_ref());

    info!("Received unverify request");

    // Validate authorization
    if !is_authorized(&headers) {
        warn!(target: "save_to_log_file", "Unauthorized unverify attempt");
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
        if ix.data == UPGRADE_INSTRUCTION_DATA {
            let program_id = &ix.accounts[1];
            info!(target: "save_to_log_file", "Processing upgrade instruction for program: {}", program_id);

            if let Err(e) = process_program_upgrade(&db, program_id).await {
                error!(target: "save_to_log_file", "Failed to process program upgrade: {}", e);
                continue;
            }
        }
    }

    (StatusCode::OK, "Unverify request received")
}

/// Processes a program upgrade by checking and updating verification status
async fn process_program_upgrade(
    db: &DbClient,
    program_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get current verification status
    let executable_hash = db.get_verified_build(program_id, None).await?;

    // Get new on-chain hash
    let onchain_hash = get_on_chain_hash(program_id).await?;

    // Check if program needs to be unverified
    if onchain_hash != executable_hash.on_chain_hash {
        info!(target: "save_to_log_file", "Program {} has been upgraded, unverifying", program_id);
        db.unverify_program(program_id, &onchain_hash).await?;
        info!(target: "save_to_log_file", "Successfully unverified program {}", program_id);
    } else {
        info!(target: "save_to_log_file", "Program {} has not been upgraded", program_id);
    }

    Ok(())
}
