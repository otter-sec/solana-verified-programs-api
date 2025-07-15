use crate::{errors::ApiError, Result, CONFIG};
use solana_account_decoder::parse_bpf_loader::{
    parse_bpf_upgradeable_loader, BpfUpgradeableLoaderAccountType, UiProgram, UiProgramData,
};
use solana_client::{
    nonblocking::rpc_client::RpcClient, rpc_client::GetConfirmedSignaturesForAddress2Config, rpc_config::RpcTransactionConfig,
};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use solana_transaction_status::{EncodedTransaction, UiMessage, UiTransactionEncoding};
use std::str::FromStr;
use tracing::{error, info};

/// Retrieves the upgrade authority for a Solana program from the blockchain
///
/// # Arguments
/// * `program_id` - Public key of the program to check
///
/// # Returns
/// * `Result<Option<String>>` - Program authority public key if it exists
///
/// This function:
/// 1. Fetches the program account data
/// 2. Extracts the program data account address
/// 3. Fetches the program data account
/// 4. Extracts the upgrade authority
pub async fn get_program_authority(program_id: &str) -> Result<(Option<String>, bool)> {
    // Parse program ID as Pubkey
    let program_id = Pubkey::from_str(program_id).map_err(|e| {
        error!("Invalid program ID: {}", e);
        ApiError::Custom(format!("Invalid program ID: {}", e))
    })?;

    let client = RpcClient::new(CONFIG.rpc_url.clone());
    info!("Fetching program authority for: {}", program_id);

    // Get program account data
    let program_account_bytes = client.get_account_data(&program_id).await.map_err(|e| {
        error!("Failed to fetch program account data: {}", e);
        ApiError::Custom(format!("Failed to fetch program account: {}", e))
    })?;

    // Parse program account to get program data address
    let program_data_account_id = match parse_bpf_upgradeable_loader(&program_account_bytes)? {
        BpfUpgradeableLoaderAccountType::Program(UiProgram { program_data }) => {
            Pubkey::from_str(&program_data).map_err(|e| {
                error!("Invalid program data address: {}", e);
                ApiError::Custom(format!("Invalid program data pubkey: {}", e))
            })?
        }
        unexpected => {
            error!("Unexpected program account type: {:?}", unexpected);
            return Err(ApiError::Custom(format!(
                "Expected Program account type, found: {:?}",
                unexpected
            )));
        }
    };

    // Fetch program data account
    match client.get_account_data(&program_data_account_id).await {
        Ok(bytes) => {
            if let BpfUpgradeableLoaderAccountType::ProgramData(UiProgramData {
                authority, ..
            }) = parse_bpf_upgradeable_loader(&bytes)?
            {
                if authority.is_some() {
                    info!("Successfully retrieved program authority: {:?}", authority);
                    // Returning authority and is_frozen as false
                    return Ok((authority, false));
                }
            }
        }
        Err(e) => {
            error!("Failed to fetch program data account: {}", e);
        }
    }

    info!(
        "Fetching program authority from latest transaction for {}",
        program_data_account_id.to_string()
    );

    // Set a limit on the number of transactions to fetch
    let config = GetConfirmedSignaturesForAddress2Config {
        limit: Some(1), // Fetch only the latest 1 transaction
        before: None,
        until: None,
        commitment: None,
    };

    // Fetch recent transactions for the program data account
    let transactions = match client
        .get_signatures_for_address_with_config(&program_data_account_id, config)
        .await
    {
        Ok(txns) => txns,
        Err(e) => {
            error!("Failed to fetch recent transactions: {}", e);
            return Ok((None, false)); // Return is_frozen as true if we can't fetch transactions
        }
    };

    // Take the latest transaction
    if let Some(latest_transaction) = transactions.first() {
        let signature = Signature::from_str(&latest_transaction.signature).map_err(|e| {
            ApiError::Custom(format!("Failed to parse transaction signature: {}", e))
        })?;

        // Fetch the full transaction details using the signature
        let transaction_details = client
            .get_transaction_with_config(
                &signature,
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Json),
                    commitment: None,
                    max_supported_transaction_version: Some(0),
                },
            )
            .await?;

        // Access and decode the accounts involved
        let versioned_transaction = transaction_details.transaction.transaction;
        if let EncodedTransaction::Json(ui_transaction) = versioned_transaction {
            if let UiMessage::Raw(raw_message) = &ui_transaction.message {
                if raw_message.account_keys.contains(&"SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf".to_string()) {
                    let authority = raw_message.account_keys[3].clone();
                    return Ok((Some(authority), true));
                }
                info!(
                    "Successfully retrieved program authority from transaction: {:?}",
                    raw_message.account_keys[0]
                );
                // Return the authority and is_frozen as true
                return Ok((Some(raw_message.account_keys[0].clone()), true));
            }
        }
    }

    Ok((None, false)) // Default to is_frozen as true if no authority is found
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_program_authority() {
        let result = get_program_authority("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC").await;

        assert!(
            result.is_ok(),
            "Failed to get authority: {:?}",
            result.err()
        );
        let authority = result.unwrap();

        assert_eq!(
            authority.0,
            Some("9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU".to_string()),
            "Unexpected authority value"
        );
    }

    #[tokio::test]
    async fn test_get_program_authority_for_frozen_program() {
        let result = get_program_authority("333UA891CYPpAJAthphPT3hg1EkUBLhNFoP9HoWW3nug").await;

        assert!(
            result.is_ok(),
            "Failed to get authority: {:?}",
            result.err()
        );
        let authority = result.unwrap();

        assert_eq!(
            authority.0,
            Some("FHKkBao61GZt3bkKbfMmd4GmDqQyYudyWQc5RUk4PKuZ".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_program_authority_for_frozen_program_1() {
        let result = get_program_authority("paxosVkYuJBKUQoZGAidRA47Qt4uidqG5fAt5kmr1nR").await;

        assert!(
            result.is_ok(),
            "Failed to get authority: {:?}",
            result.err()
        );
        let authority = result.unwrap();

        assert_eq!(
            authority.0,
            Some("6EqYa8BxABzh5qHXYGw3nAoAueCyZG6KMG7K9WTA23sD".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_program_authority_invalid_program() {
        let invalid_program = Pubkey::new_unique();
        let result = get_program_authority(&invalid_program.to_string()).await;
        assert!(result.is_err(), "Expected error for invalid program");
    }
}
