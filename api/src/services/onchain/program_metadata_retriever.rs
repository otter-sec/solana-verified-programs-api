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

/// Program ID for the Otter Verify program
pub const OTTER_VERIFY_PROGRAMID: Pubkey =
    solana_sdk::pubkey!("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC");

/// Whitelisted signer public keys
pub const SIGNER_KEYS: [Pubkey; 2] = [
    solana_sdk::pubkey!("9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU"),
    solana_sdk::pubkey!("CyJj5ejJAUveDXnLduJbkvwjxcmWJNqCuB9DR7AExrHn"),
];

/// Build parameters stored in Otter Verify PDA
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

/// Implementation of helper functions for OtterBuildParams
impl OtterBuildParams {
    /// Checks if the build has BPF flag enabled
    pub fn is_bpf(&self) -> bool {
        self.args.iter().any(|arg| arg == "--bpf")
    }

    /// Gets the mount path from build arguments
    pub fn get_mount_path(&self) -> Option<String> {
        self.args
            .iter()
            .position(|arg| arg == "--mount-path")
            .map(|index| self.args[index + 1].clone())
    }

    /// Gets the library name from build arguments
    pub fn get_library_name(&self) -> Option<String> {
        self.args
            .iter()
            .position(|arg| arg == "--library-name")
            .map(|index| self.args[index + 1].clone())
    }

    /// Gets the base image from build arguments
    pub fn get_base_image(&self) -> Option<String> {
        self.args
            .iter()
            .position(|arg| arg == "--base-image" || arg == "-b")
            .map(|index| self.args[index + 1].clone())
    }

    /// Gets additional cargo arguments which are after "--"
    pub fn get_cargo_args(&self) -> Option<Vec<String>> {
        self.args
            .iter()
            .position(|arg| arg == "--")
            .map(|index| self.args[index + 1..].to_vec())
    }
}

/// Retrieves Otter Verify PDA for a program and signer
/// 
/// This function is used to retrieve the OtterVerify PDA for a given program and signer.
/// It uses the seeds "otter_verify", the signer's public key, and the program's public key to calculate the PDA.
/// It then fetches the account data for the PDA and attempts to deserialize it into an `OtterBuildParams` struct.
/// 
/// # Arguments
/// 
/// * `client` - The RPC client to use for fetching account data
/// * `signer` - The PDA signer's public key
/// * `program_id_pubkey` - The program's public key
/// 
/// # Returns
/// 
/// * `Result<OtterBuildParams>` - The OtterVerify PDA parameters if successful, or an error
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
    OtterBuildParams::try_from_slice(&account_data[8..])
        .map_err(|e| ApiError::Custom(format!("Failed to deserialize PDA data: {}", e)))
}

/// Retrieves Otter Verify parameters for a program
/// 
/// This function is used to retrieve the OtterVerify parameters for a given program.
/// It tries to retrieve the parameters from the PDA first if provided, then from the program authority, and finally from the whitelisted signers.
/// If no valid parameters are found, it returns an error.
/// 
/// # Arguments
/// 
/// * `program_id` - The program's public key
/// * `signer` - The PDA signer's public key
/// * `program_authority` - The program's authority public key
/// 
/// # Returns
/// 
/// * `Result<(OtterBuildParams, String)>` - The OtterVerify parameters and the signer's public key if successful, or an error
pub async fn get_otter_verify_params(
    program_id: &str,
    signer: Option<String>,
    program_authority: Option<String>,
) -> Result<(OtterBuildParams, String)> {
    let client = RpcClient::new(CONFIG.rpc_url.clone());
    let program_id_pubkey = Pubkey::from_str(program_id)?;

    // Try with provided signer
    if let Some(signer) = signer {
        let signer_pubkey = Pubkey::from_str(&signer)
            .map_err(|_| ApiError::Custom(format!("Invalid signer pubkey: {}", signer)))?;
        if let Ok(params) = get_otter_pda(&client, &signer_pubkey, &program_id_pubkey).await {
            return Ok((params, signer_pubkey.to_string()));
        }
        return Err(ApiError::Custom(format!(
            "Otter-Verify PDA not found for signer: {}",
            signer
        )));
    }

    // Try with program authority
    if let Some(authority) = &program_authority {
        let authority_pubkey = Pubkey::from_str(authority)?;
        if let Ok(params) = get_otter_pda(&client, &authority_pubkey, &program_id_pubkey).await {
            return Ok((params, authority_pubkey.to_string()));
        }
    }

    // Try with whitelisted signers
    for signer in SIGNER_KEYS.iter() {
        if let Ok(params) = get_otter_pda(&client, signer, &program_id_pubkey).await {
            return Ok((params, signer.to_string()));
        }
    }

    // If no valid parameters are found, return an error
    Err(ApiError::Custom(
        "No valid Otter-Verify PDA found".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::SolanaProgramBuildParams;

    #[tokio::test]
    async fn test_get_otter_verify_params() {
        let program_id = "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC";
        let result = get_otter_verify_params(program_id, None, None).await;
        assert!(result.is_ok(), "Failed to get params: {:?}", result.err());

        let (params, _) = result.unwrap();
        assert_eq!(
            params.address.to_string(),
            "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC"
        );
        assert_eq!(
            params.signer.to_string(),
            "9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU"
        );
    }

    #[tokio::test]
    async fn test_build_params_conversion() {
        let program_id = "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu";
        let result = get_otter_verify_params(program_id, None, None).await;
        assert!(result.is_ok(), "Failed to get params: {:?}", result.err());

        let (params, _) = result.unwrap();
        let build_params = SolanaProgramBuildParams::from(params);

        assert_eq!(build_params.program_id, program_id);
        assert!(build_params.lib_name.unwrap() == "squads_mpl");
        assert!(build_params.bpf_flag.unwrap());
    }
}
