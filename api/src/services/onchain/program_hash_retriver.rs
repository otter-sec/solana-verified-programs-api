// src/services/onchain/program_hash_retriver.rs

use crate::errors::ApiError;
use crate::services::misc::get_last_line;
use crate::{Result, CONFIG};
use tokio::process::Command;

pub async fn get_on_chain_hash(program_id: &str) -> Result<String> {
    let rpc_url = CONFIG.rpc_url.clone();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_on_chain_hash() {
        let program_id = "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC";
        let data = get_on_chain_hash(program_id).await;
        assert!(data.is_ok());
        let hash = data.unwrap();
        assert!(hash == "c117c3610fca94c5be64eed41e4f2f6783a38b493b245207f3d7e3d7a63ae8e0");
    }
}
