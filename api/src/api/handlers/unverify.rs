use crate::{
    db::{models::UpgradeProgramInstruction, DbClient},
    services::get_on_chain_hash,
    CONFIG,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::Value;
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
    info!("Received unverify request");

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
        if ix.data == UPGRADE_INSTRUCTION_DATA {
            let program_id = &ix.accounts[1];
            info!("Processing upgrade instruction for program: {}", program_id);

            if let Err(e) = process_program_upgrade(&db, program_id).await {
                error!("Failed to process program upgrade: {}", e);
                continue;
            }
        }
    }

    (StatusCode::OK, "Unverify request received")
}

/// Validates the authorization header against the configured secret
fn is_authorized(headers: &HeaderMap) -> bool {
    headers
        .get("AUTHORIZATION")
        .and_then(|value| value.to_str().ok())
        .map_or(false, |header_value| header_value == CONFIG.auth_secret)
}

/// Extracts and validates the upgrade instruction from the payload
fn extract_instruction(
    payload: &[Value],
) -> Result<UpgradeProgramInstruction, (StatusCode, &'static str)> {
    if payload.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Empty payload"));
    }

    serde_json::from_value(payload[0].clone()).map_err(|e| {
        error!("Failed to parse instruction payload: {}", e);
        (StatusCode::BAD_REQUEST, "Invalid payload")
    })
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
        info!("Program {} has been upgraded, unverifying", program_id);
        db.unverify_program(program_id, &onchain_hash).await?;
        info!("Successfully unverified program {}", program_id);
    } else {
        info!("Program {} has not been upgraded", program_id);
    }

    Ok(())
}
