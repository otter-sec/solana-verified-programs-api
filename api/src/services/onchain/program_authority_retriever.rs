use std::str::FromStr;

use crate::{Result, CONFIG};
use solana_account_decoder::parse_bpf_loader::{
    parse_bpf_upgradeable_loader, BpfUpgradeableLoaderAccountType, UiProgram, UiProgramData,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

pub async fn get_program_authority(program_id: &Pubkey) -> Result<Option<String>> {
    let rpc_url = CONFIG.rpc_url.clone();
    let client = RpcClient::new(rpc_url);

    let program_account_bytes = client.get_account_data(program_id).await?;

    let program_data_account_id = match parse_bpf_upgradeable_loader(&program_account_bytes)? {
        BpfUpgradeableLoaderAccountType::Program(UiProgram { program_data }) => {
            Pubkey::from_str(&program_data)?
        }
        unexpected => {
            return Err(crate::errors::ApiError::Custom(format!(
                "Unexpected program account type: {:?}",
                unexpected
            )));
        }
    };

    let program_data_account_bytes = client.get_account_data(&program_data_account_id).await?;

    let program_data = match parse_bpf_upgradeable_loader(&program_data_account_bytes)? {
        BpfUpgradeableLoaderAccountType::ProgramData(UiProgramData { authority, .. }) => authority,
        unexpected => {
            return Err(crate::errors::ApiError::Custom(format!(
                "Unexpected program data account type: {:?}",
                unexpected
            )));
        }
    };
    Ok(program_data)
}

// tests
#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;

    #[tokio::test]
    async fn test_get_program_authority() {
        let program_id = Pubkey::from_str("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC").unwrap();
        let result = get_program_authority(&program_id).await;
        assert!(result.is_ok());
        let authority = result.unwrap();
        assert_eq!(
            authority,
            Some("9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU".to_string())
        );
    }
}
