use crate::{errors::ApiError, Result, CONFIG};
use solana_account_decoder::parse_bpf_loader::{
    parse_bpf_upgradeable_loader, BpfUpgradeableLoaderAccountType, UiProgram, UiProgramData,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::{error, info};

/// Retrieves the upgrade authority for a Solana program
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
pub async fn get_program_authority(program_id: &str) -> Result<Option<String>> {
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

    // Get program data account
    let program_data_account_bytes = client
        .get_account_data(&program_data_account_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch program data account: {}", e);
            ApiError::Custom(format!("Failed to fetch program data account: {}", e))
        })?;

    // Parse program data account to get authority
    match parse_bpf_upgradeable_loader(&program_data_account_bytes)? {
        BpfUpgradeableLoaderAccountType::ProgramData(UiProgramData { authority, .. }) => {
            info!("Successfully retrieved program authority: {:?}", authority);
            Ok(authority)
        }
        unexpected => {
            error!("Unexpected program data account type: {:?}", unexpected);
            Err(ApiError::Custom(format!(
                "Expected ProgramData account type, found: {:?}",
                unexpected
            )))
        }
    }
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
            authority,
            Some("9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU".to_string()),
            "Unexpected authority value"
        );
    }

    #[tokio::test]
    async fn test_get_program_authority_invalid_program() {
        let invalid_program = Pubkey::new_unique();
        let result = get_program_authority(&invalid_program.to_string()).await;
        assert!(result.is_err(), "Expected error for invalid program");
    }
}
