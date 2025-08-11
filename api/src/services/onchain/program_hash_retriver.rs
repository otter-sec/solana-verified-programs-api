// src/services/onchain/program_hash_retriver.rs

use std::time::Duration;

use crate::errors::ApiError;
use crate::services::misc::get_last_line;
use crate::{Result, CONFIG};
use tokio::process::Command;
use tokio::time::sleep;
use tracing::{error, info};

/// Retrieves the on-chain hash for a Solana program
///
/// # Arguments
/// * `program_id` - Address of the program to check
///
/// # Returns
/// * `Result<String>` - Program hash if successful
///
/// Makes up to 3 attempts to retrieve the hash, with exponential backoff delays (2, 4, 8 seconds).
pub async fn get_on_chain_hash(program_id: &str) -> Result<String> {
    let rpc_url = CONFIG.rpc_url.clone();
    let mut cmd = Command::new("solana-verify");
    cmd.arg("get-program-hash")
        .arg(program_id)
        .arg("--url")
        .arg(rpc_url);

    info!(
        "Attempting to get on-chain hash for program: {}",
        program_id
    );

    for attempt in 1..=3 {
        match execute_command(&mut cmd).await {
            Ok(hash) => {
                info!(
                    "Successfully retrieved hash for program {}: {}",
                    program_id, hash
                );
                return Ok(hash);
            }
            Err(e) => {
                let error_str = e.to_string();
                // Don't retry if the program appears to be closed - return immediately
                if error_str.contains("Program appears to be closed") {
                    error!("Program {} appears to be closed, not retrying", program_id);
                    return Err(e);
                }

                error!(
                    "Attempt {}/3 failed to get on-chain hash for {}: {}",
                    attempt, program_id, e
                );
                if attempt < 3 {
                    // Exponential backoff: 2^attempt seconds (2, 4, 8...)
                    let delay = Duration::from_secs(2_u64.pow(attempt));
                    sleep(delay).await;
                }
            }
        }
    }

    Err(ApiError::Custom(
        "Failed to get on-chain hash after 3 attempts".to_string(),
    ))
}

/// Executes the solana-verify command and processes its output
async fn execute_command(cmd: &mut Command) -> Result<String> {
    let output = cmd
        .output()
        .await
        .map_err(|e| ApiError::Custom(format!("Failed to execute solana-verify command: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check if the error indicates a closed/non-existent program data account
        if stderr.contains("Could not find program data") {
            return Err(ApiError::Custom(
                "Program appears to be closed - program data account not found".to_string(),
            ));
        }
        return Err(ApiError::Custom(format!("Command failed: {stderr}")));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| ApiError::Custom(format!("Failed to parse command output: {e}")))?;

    get_last_line(&stdout)
        .ok_or_else(|| ApiError::Custom("Failed to extract hash from command output".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_on_chain_hash() {
        let program_id = "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC";
        let result = get_on_chain_hash(program_id).await;

        assert!(result.is_ok(), "Failed to get hash: {:?}", result.err());
        let hash = result.unwrap();
        assert_eq!(
            hash, "c117c3610fca94c5be64eed41e4f2f6783a38b493b245207f3d7e3d7a63ae8e0",
            "Unexpected hash value"
        );
    }

    #[tokio::test]
    async fn test_get_on_chain_hash_closed_program() {
        // This program has been closed - program data account no longer exists
        let program_id = "2gFsaXeN9jngaKbQvZsLwxqfUrT2n4WRMraMpeL8NwZM";
        let result = get_on_chain_hash(program_id).await;

        assert!(result.is_err(), "Should return error for closed program");
        let error = result.err().unwrap();
        let error_str = error.to_string();
        assert!(
            error_str.contains("Program appears to be closed")
                || error_str.contains("Could not find program data"),
            "Error should indicate program is closed: {error_str}"
        );
    }
}
