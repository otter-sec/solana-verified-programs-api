// src/services/chain.rs

use std::env;
use tokio::process::Command;
use crate::errors::ApiError;
use crate::services::misc::get_last_line;
use crate::Result;

pub async fn get_on_chain_hash(program_id: &str) -> Result<String> {
    let rpc_url =
        env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let mut cmd = Command::new("solana-verify");
    cmd.arg("get-program-hash").arg(program_id);
    cmd.arg("--url").arg(rpc_url);

    let output = cmd
        .output()
        .await
        .map_err(|_| ApiError::Custom("Failed to run process get-program-hash".to_string()))?;

    if !output.status.success() {
        tracing::error!(
            "Failed to get on-chain hash {}",
            String::from_utf8(output.stderr)?
        );
        return Err(ApiError::Custom("Failed to get on-chain hash".to_string()));
    }
    let result = String::from_utf8(output.stdout)?;
    let hash = get_last_line(&result).ok_or_else(|| {
        ApiError::Custom("Failed to build and get output from program".to_string())
    })?;
    Ok(hash)
}
