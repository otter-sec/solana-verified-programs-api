use std::str::FromStr;

use crate::{errors::ApiError, Result, CONFIG};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

#[cfg(feature = "use-external-pdas")]
use {
    solana_account_decoder::UiAccountEncoding,
    solana_client::{
        rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
        rpc_filter::{Memcmp, RpcFilterType},
    },
    solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel},
};

use super::get_program_authority;

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct OtterBuildParams {
    pub address: Pubkey,
    pub signer: Pubkey,
    pub version: String,
    pub git_url: String,
    pub commit: String,
    pub args: Vec<String>,
    pub deployed_slot: u64,
    bump: u8,
}

const OTTER_VERIFY_PROGRAMID: Pubkey =
    solana_sdk::pubkey!("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC");

const SIGNER_KEYS: [Pubkey; 2] = [
    solana_sdk::pubkey!("9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU"),
    solana_sdk::pubkey!("CyJj5ejJAUveDXnLduJbkvwjxcmWJNqCuB9DR7AExrHn"),
];

impl OtterBuildParams {
    pub fn is_bpf(&self) -> bool {
        self.args.iter().any(|arg| arg == "--bpf")
    }

    // get mount-path i.e arg after mount-path
    pub fn get_mount_path(&self) -> Option<String> {
        let mount_path = self.args.iter().position(|arg| arg == "--mount-path");
        if let Some(index) = mount_path {
            return Some(self.args[index + 1].clone());
        }
        None
    }

    // get --library-name i.e arg after --library-name
    pub fn get_library_name(&self) -> Option<String> {
        let library_name = self.args.iter().position(|arg| arg == "--library-name");
        if let Some(index) = library_name {
            return Some(self.args[index + 1].clone());
        }
        None
    }

    pub fn get_base_image(&self) -> Option<String> {
        let base_image = self
            .args
            .iter()
            .position(|arg| arg == "--base-image" || arg == "-b");
        if let Some(index) = base_image {
            return Some(self.args[index + 1].clone());
        }
        None
    }

    pub fn get_cargo_args(&self) -> Option<Vec<String>> {
        let cargo_args = self.args.iter().position(|arg| arg == "--");
        if let Some(index) = cargo_args {
            return Some(self.args[index + 1..].to_vec());
        }
        None
    }
}

pub async fn get_otter_pda(
    client: &RpcClient,
    signer: &Pubkey,
    program_id_pubkey: &Pubkey,
) -> Result<OtterBuildParams> {
    let seeds: &[&[u8]] = &[
        b"otter_verify",
        &signer.to_bytes(),
        &program_id_pubkey.to_bytes(),
    ];
    let (pda_account, _) = Pubkey::find_program_address(seeds, &OTTER_VERIFY_PROGRAMID);
    let account_data = client.get_account_data(&pda_account).await?;
    let otter_build_params = OtterBuildParams::try_from_slice(&account_data[8..])?;
    Ok(otter_build_params)
}

#[cfg(feature = "use-external-pdas")]
pub async fn get_all_pdas_availabe(
    client: &RpcClient,
    program_id_pubkey: &Pubkey,
) -> Result<OtterBuildParams> {
    let filter = vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
        8,
        &program_id_pubkey.to_bytes(),
    ))];

    let config = RpcProgramAccountsConfig {
        filters: Some(filter),
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            data_slice: None,
            commitment: Some(CommitmentConfig {
                commitment: CommitmentLevel::Confirmed,
            }),
            min_context_slot: None,
        },
        with_context: None,
        sort_results: Some(true),
    };

    let accounts = client
        .get_program_accounts_with_config(&OTTER_VERIFY_PROGRAMID, config)
        .await?;

    for account in accounts {
        let otter_build_params = OtterBuildParams::try_from_slice(&account.1.data[8..]);
        if let Ok(otter_build_params) = otter_build_params {
            return Ok(otter_build_params);
        }
    }

    Err(ApiError::Custom("Otter-Verify PDA not found".to_string()))
}

pub async fn get_otter_verify_params(
    program_id: &str,
    signer: Option<String>,
) -> Result<(OtterBuildParams, String)> {
    let rpc_url = CONFIG.rpc_url.clone();
    let client = RpcClient::new(rpc_url.clone());
    let program_id_pubkey = Pubkey::from_str(program_id)?;

    if let Some(signer) = signer {
        let signer_pubkey = Pubkey::from_str(&signer)
            .map_err(|_| ApiError::Custom(format!("Invalid signer pubkey: {}", signer)))?;

        if let Ok(otter_build_params) =
            get_otter_pda(&client, &signer_pubkey, &program_id_pubkey).await
        {
            return Ok((otter_build_params, signer_pubkey.to_string()));
        } else {
            return Err(ApiError::Custom(format!(
                "Otter-Verify PDA not found for the given signer: {}",
                signer
            )));
        }
    }

    // Try the first PDA based on authority
    if let Some(authority) = get_program_authority(&program_id_pubkey).await? {
        let authority_pubkey = Pubkey::from_str(&authority)?;

        if let Ok(otter_build_params) =
            get_otter_pda(&client, &authority_pubkey, &program_id_pubkey).await
        {
            return Ok((otter_build_params, authority_pubkey.to_string()));
        }
    }

    // Fallback: PDA based on whitelisted pubkeys
    for signer in SIGNER_KEYS.iter() {
        if let Ok(otter_build_params) = get_otter_pda(&client, signer, &program_id_pubkey).await {
            return Ok((otter_build_params, signer.to_string()));
        }
    }

    // Fallback: get PDA accounts fro the given program id
    #[cfg(feature = "use-external-pdas")]
    if let Ok(otter_build_params) = get_all_pdas_availabe(&client, &program_id_pubkey).await {
        return Ok(otter_build_params);
    }

    Err(ApiError::Custom("Otter-Verify PDA not found".to_string()))
}

#[cfg(test)]
mod tests {
    use crate::db::models::SolanaProgramBuildParams;

    use super::*;

    #[tokio::test]
    async fn test_get_on_chain_hash() {
        let program_id = "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC";
        let data = get_otter_verify_params(program_id, None).await;
        assert!(data.is_ok());
        let params = data.unwrap().0;
        assert!(params.address.to_string() == "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC");
        assert!(params.signer.to_string() == "9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU");
    }

    #[tokio::test]
    async fn test_params() {
        let program_id = "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC";
        let data = get_otter_verify_params(program_id, None).await;
        assert!(data.is_ok());
        let params = data.unwrap().0;
        let solana_build_params = SolanaProgramBuildParams::from(params);
        assert!(solana_build_params.program_id == "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC");
        assert!(solana_build_params.base_image.is_some());
        assert!(solana_build_params.mount_path.is_none());
        assert!(solana_build_params.lib_name.is_some());
        assert!(solana_build_params.lib_name.unwrap() == "otter_verify");
        assert!(!solana_build_params.bpf_flag.unwrap());
        assert!(solana_build_params.cargo_args.is_none());
    }

    #[tokio::test]
    async fn test_params_squads() {
        let program_id = "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu";
        let data = get_otter_verify_params(program_id, None).await;
        assert!(data.is_ok());
        let params = data.unwrap().0;
        let solana_build_params = SolanaProgramBuildParams::from(params);
        assert!(solana_build_params.program_id == "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu");
        assert!(solana_build_params.base_image.is_none());
        assert!(solana_build_params.mount_path.is_none());
        assert!(solana_build_params.lib_name.is_some());
        assert!(solana_build_params.lib_name.unwrap() == "squads_mpl");
        assert!(solana_build_params.bpf_flag.unwrap());
    }
}
