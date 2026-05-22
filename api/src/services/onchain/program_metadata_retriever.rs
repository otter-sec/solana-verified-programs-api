use std::str::FromStr;

use crate::{
    errors::{ApiError, Result},
    validation::Address,
};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;

/// Program ID for the Otter Verify program
pub const OTTER_VERIFY_PROGRAM_ID: Pubkey =
    solana_pubkey::pubkey!("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC");

/// Whitelisted signer public keys
pub const SIGNER_KEYS: [Pubkey; 3] = [
    solana_pubkey::pubkey!("9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU"),
    solana_pubkey::pubkey!("CyJj5ejJAUveDXnLduJbkvwjxcmWJNqCuB9DR7AExrHn"),
    solana_pubkey::pubkey!("5vJwnLeyjV8uNJSp1zn7VLW8GwiQbcsQbGaVSwRmkE4r"),
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
        self.arg_value("--mount-path")
    }

    /// Gets the library name from build arguments
    pub fn get_library_name(&self) -> Option<String> {
        self.arg_value("--library-name")
    }

    /// Gets the base image from build arguments
    pub fn get_base_image(&self) -> Option<String> {
        self.arg_value("--base-image")
            .or_else(|| self.arg_value("-b"))
    }

    /// Gets additional cargo arguments which are after "--"
    pub fn get_cargo_args(&self) -> Option<Vec<String>> {
        self.args
            .iter()
            .position(|arg| arg == "--")
            .map(|index| self.args[index + 1..].to_vec())
    }

    /// Gets the architecture from build arguments
    pub fn get_arch(&self) -> Option<String> {
        self.arg_value("--arch")
    }

    /// Returns the value following `flag` in `args`, or `None` if the flag
    /// is missing or has no following value (last entry).
    fn arg_value(&self, flag: &str) -> Option<String> {
        let idx = self.args.iter().position(|a| a == flag)?;
        self.args.get(idx + 1).cloned()
    }

    /// `solana-verify --cargo-build-sbf-args` value
    pub fn get_cargo_build_sbf_args(&self) -> Option<String> {
        const PREFIX: &str = "--cargo-build-sbf-args";
        for (i, arg) in self.args.iter().enumerate() {
            if arg == PREFIX {
                return self.args.get(i + 1).map(|v| strip_surrounding_quotes(v));
            }
            if let Some(value) = arg
                .strip_prefix(PREFIX)
                .and_then(|rest| rest.strip_prefix('='))
            {
                return Some(strip_surrounding_quotes(value));
            }
        }
        None
    }
}

fn strip_surrounding_quotes(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
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
    let (pda_account, _) = Pubkey::find_program_address(seeds, &OTTER_VERIFY_PROGRAM_ID);
    let account_data = client.get_account_data(&pda_account).await?;
    // Skip the 8-byte Anchor discriminator.
    let body = account_data
        .get(8..)
        .ok_or_else(|| ApiError::Custom("PDA account data is too short".to_string()))?;
    OtterBuildParams::try_from_slice(body)
        .map_err(|e| ApiError::Custom(format!("Failed to deserialize PDA data: {e}")))
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
/// * `Result<(OtterBuildParams, Address)>` - The OtterVerify parameters and the signer if successful, or an error
pub async fn get_otter_verify_params(
    rpc: &RpcClient,
    program_id: &str,
    signer: Option<String>,
    program_authority: Option<String>,
) -> Result<(OtterBuildParams, Address)> {
    let program_id_pubkey = Pubkey::from_str(program_id)?;

    // Try with provided signer
    if let Some(signer) = signer {
        let signer_pubkey = Pubkey::from_str(&signer)
            .map_err(|_| ApiError::BadRequest(format!("Invalid signer pubkey: {signer}")))?;
        if let Ok(params) = get_otter_pda(rpc, &signer_pubkey, &program_id_pubkey).await {
            return Ok((params, Address(signer_pubkey)));
        }
        return Err(ApiError::NotFound(format!(
            "Otter-Verify PDA not found for signer: {signer}"
        )));
    }

    // Try with program authority
    if let Some(authority) = &program_authority {
        let authority_pubkey = Pubkey::from_str(authority)?;
        if let Ok(params) = get_otter_pda(rpc, &authority_pubkey, &program_id_pubkey).await {
            return Ok((params, Address(authority_pubkey)));
        }
    }

    // Try with whitelisted signers
    for signer in SIGNER_KEYS.iter() {
        if let Ok(params) = get_otter_pda(rpc, signer, &program_id_pubkey).await {
            return Ok((params, Address(*signer)));
        }
    }

    // If no valid parameters are found, return an error
    Err(ApiError::NotFound(
        "No valid Otter-Verify PDA found".to_string(),
    ))
}

/// Returns `true` only if the program-data account doesn't exist
/// (`AccountNotFound`). Any other error -- RPC down, rate-limited -- is
/// treated as "exists" so we don't accidentally flip a healthy program
/// to closed on a transient hiccup.
pub async fn is_program_data_missing(rpc: &RpcClient, program_id: &str) -> bool {
    let Ok(program_id_pubkey) = Pubkey::from_str(program_id) else {
        return false;
    };
    let program_data_pda =
        Pubkey::find_program_address(&[program_id_pubkey.as_ref()], &bpf_loader_upgradeable::id())
            .0;

    match rpc.get_account(&program_data_pda).await {
        Ok(_) => false,
        Err(err) => err.to_string().contains("AccountNotFound"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::NewBuild;

    fn rpc() -> RpcClient {
        RpcClient::new("https://api.mainnet-beta.solana.com".to_string())
    }

    #[tokio::test]
    #[ignore = "hits mainnet RPC"]
    async fn test_get_otter_verify_params() {
        let program_id = "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC";
        let result = get_otter_verify_params(&rpc(), program_id, None, None).await;
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
    #[ignore = "hits mainnet RPC"]
    async fn test_build_params_conversion() {
        let program_id = "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu";
        let result = get_otter_verify_params(&rpc(), program_id, None, None).await;
        assert!(result.is_ok(), "Failed to get params: {:?}", result.err());

        let (params, _) = result.unwrap();
        let new_build = NewBuild::from(&params);

        assert_eq!(new_build.program_id.to_string(), program_id);
        assert_eq!(new_build.lib_name.as_deref(), Some("squads_mpl"));
        assert!(new_build.bpf_flag);
    }

    #[test]
    fn get_cargo_build_sbf_args_from_token_pda_args() {
        let otter = OtterBuildParams {
            address: Pubkey::default(),
            signer: Pubkey::default(),
            version: String::new(),
            git_url: String::new(),
            commit: String::new(),
            args: vec![
                "--library-name".to_string(),
                "pinocchio_token_program".to_string(),
                "--base-image".to_string(),
                "solanafoundation/solana-verifiable-build:3.1.9".to_string(),
                "--cargo-build-sbf-args=\"--tools-version v1.54\"".to_string(),
            ],
            deployed_slot: 0,
            bump: 0,
        };
        assert_eq!(
            otter.get_cargo_build_sbf_args().as_deref(),
            Some("--tools-version v1.54")
        );
    }

    #[test]
    fn get_cargo_build_sbf_args_two_token_form() {
        let otter = OtterBuildParams {
            address: Pubkey::default(),
            signer: Pubkey::default(),
            version: String::new(),
            git_url: String::new(),
            commit: String::new(),
            args: vec![
                "--cargo-build-sbf-args".to_string(),
                "\"--tools-version v1.54\"".to_string(),
            ],
            deployed_slot: 0,
            bump: 0,
        };
        assert_eq!(
            otter.get_cargo_build_sbf_args().as_deref(),
            Some("--tools-version v1.54")
        );
    }

    #[tokio::test]
    #[ignore = "hits mainnet RPC"]
    async fn test_program_buffer_missing() {
        let program_id = "2gFsaXeN9jngaKbQvZsLwxqfUrT2n4WRMraMpeL8NwZM";
        assert!(is_program_data_missing(&rpc(), program_id).await);
    }
}
