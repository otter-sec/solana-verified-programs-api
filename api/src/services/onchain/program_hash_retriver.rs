// src/services/onchain/program_hash_retriver.rs

use std::time::Duration;

use crate::errors::ApiError;
use crate::services::misc::get_last_line;
use crate::{Result, CONFIG};
use tokio::process::Command;
use tokio::time::sleep;

pub async fn get_on_chain_hash(program_id: &str) -> Result<String> {
    let rpc_url = CONFIG.rpc_url.clone();
    let mut cmd = Command::new("solana-verify");
    cmd.arg("get-program-hash").arg(program_id);
    cmd.arg("--url").arg(rpc_url);

    for attempt in 1..=3 {
        match cmd.output().await {
            Ok(output) => {
                if output.status.success() {
                    match String::from_utf8(output.stdout) {
                        Ok(result) => {
                            if let Some(hash) = get_last_line(&result) {
                                return Ok(hash);
                            } else {
                                return Err(ApiError::Custom(
                                    "Failed to build and get output from program".to_string(),
                                ));
                            }
                        }
                        Err(_) => {
                            tracing::error!("Attempt {}/3: Failed to parse output", attempt);
                        }
                    }
                } else {
                    tracing::error!(
                        "Attempt {}/3: Failed to get on-chain hash: {}",
                        attempt,
                        String::from_utf8(output.stderr.clone()).unwrap_or_default()
                    );
                }
            }
            Err(_) => {
                tracing::error!(
                    "Attempt {}/3: Failed to run process get-program-hash",
                    attempt
                );
            }
        }

        if attempt < 3 {
            sleep(Duration::from_secs(5)).await;
        }
    }

    Err(ApiError::Custom("Failed to get on-chain hash".to_string()))
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
