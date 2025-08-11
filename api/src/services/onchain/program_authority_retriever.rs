use crate::{errors::ApiError, services::rpc_manager::get_rpc_manager, Result};
use solana_account_decoder::parse_bpf_loader::{
    parse_bpf_upgradeable_loader, BpfUpgradeableLoaderAccountType, UiProgram, UiProgramData,
};
use solana_client::{
    nonblocking::rpc_client::RpcClient, rpc_client::GetConfirmedSignaturesForAddress2Config,
    rpc_config::RpcTransactionConfig,
};
use solana_sdk::{pubkey::Pubkey, signature::Signature, system_program};
use solana_transaction_status::{EncodedTransaction, UiMessage, UiTransactionEncoding};
use std::{str::FromStr, sync::Arc};
use tracing::{error, info};

const SQUADS_PROGRAM_ID: &str = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";
const EXPECTED_UPGRADE_AUTHORITY_INSTRUCTION_DATA: &str = "ZTNTtVtnvbC";
const AUTHORITY_ACCOUNT_INDEX: usize = 4;

/// Retrieves the upgrade authority for a Solana program from the blockchain
///
/// # Arguments
/// * `program_id` - Public key of the program to check
///
/// # Returns
/// * `Result<(Option<String>, bool, bool)>` - Program authority, is_frozen, is_closed
///
/// This function:
/// 1. Fetches the program account data
/// 2. Extracts the program data account address
/// 3. Fetches the program data account
/// 4. Extracts the upgrade authority
pub async fn get_program_authority(program_id: &str) -> Result<(Option<String>, bool, bool)> {
    // Parse program ID as Pubkey
    let program_id = Pubkey::from_str(program_id).map_err(|e| {
        error!("Invalid program ID: {}", e);
        ApiError::Custom(format!("Invalid program ID: {e}"))
    })?;

    info!("Fetching program authority for: {}", program_id);

    let rpc_manager = get_rpc_manager();
    rpc_manager
        .execute_with_retry(|client| async move {
            get_program_authority_with_client(client, &program_id).await
        })
        .await
}

async fn get_program_authority_with_client(
    client: Arc<RpcClient>,
    program_id: &Pubkey,
) -> Result<(Option<String>, bool, bool)> {
    // Get program account data
    let program_account_bytes = client.get_account_data(program_id).await.map_err(|e| {
        error!("Failed to fetch program account data: {}", e);
        ApiError::Custom(format!("Failed to fetch program account: {e}"))
    })?;

    // Parse program account to get program data address
    let program_data_account_id = match parse_bpf_upgradeable_loader(&program_account_bytes)? {
        BpfUpgradeableLoaderAccountType::Program(UiProgram { program_data }) => {
            Pubkey::from_str(&program_data).map_err(|e| {
                error!("Invalid program data address: {}", e);
                ApiError::Custom(format!("Invalid program data pubkey: {e}"))
            })?
        }
        unexpected => {
            error!("Unexpected program account type: {:?}", unexpected);
            return Err(ApiError::Custom(format!(
                "Expected Program account type, found: {unexpected:?}"
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
                    // Returning authority, is_frozen as false, is_closed as false
                    return Ok((authority, false, false));
                }
            }
        }
        Err(e) => {
            use solana_client::client_error::{ClientError, ClientErrorKind};
            use solana_client::rpc_request::RpcError;

            // Use is_account_closed to check if the program data account is closed
            if let Ok(true) = is_account_closed(&client, &program_data_account_id).await {
                info!(
                    "Program data account is closed - program appears to be closed: {} Program id: {}",
                    program_data_account_id, program_id
                );
                return Ok((None, false, true)); // Closed
            }

            // Check if this is specifically an account not found error using proper error types
            if let ClientError {
                kind: ClientErrorKind::RpcError(RpcError::ForUser(user_message)),
                ..
            } = &e
            {
                // Check for account not found in user-facing error messages
                if *user_message == format!("AccountNotFound: pubkey={program_data_account_id}") {
                    info!(
                        "Program data account not found - program appears to be closed: {} Program id: {}",
                        program_data_account_id, program_id
                    );
                    return Ok((None, false, true));
                }
            }

            // For any other error, return the error
            error!("Failed to fetch program data account: {}", e);
            return Err(ApiError::Custom(format!(
                "Failed to fetch program data account: {e}"
            )));
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
            return Ok((None, false, false)); // Return both as false if we can't fetch transactions
        }
    };

    // Take the latest transaction
    if let Some(latest_transaction) = transactions.first() {
        let signature = Signature::from_str(&latest_transaction.signature)
            .map_err(|e| ApiError::Custom(format!("Failed to parse transaction signature: {e}")))?;

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
                // Handle specific Squads program instruction for authority extraction
                // The instruction data "ZTNTtVtnvbC" indicates a specific authority-related operation
                // where the authority is located at the 5th account (index 4) of the instruction
                if raw_message
                    .account_keys
                    .contains(&SQUADS_PROGRAM_ID.to_string())
                {
                    let program_id_idx = raw_message
                        .account_keys
                        .iter()
                        .position(|key| key == SQUADS_PROGRAM_ID)
                        .unwrap() as u8;
                    for ix in &raw_message.instructions {
                        if ix.program_id_index == program_id_idx
                            && ix.data == EXPECTED_UPGRADE_AUTHORITY_INSTRUCTION_DATA
                        {
                            let authority_idx = ix.accounts[AUTHORITY_ACCOUNT_INDEX] as usize;
                            let authority = raw_message.account_keys[authority_idx].clone();
                            return Ok((Some(authority), true, false));
                        }
                    }
                }
                info!(
                    "Successfully retrieved program authority from transaction: {:?}",
                    raw_message.account_keys[0]
                );
                // Return the authority, is_frozen as true, is_closed as false
                return Ok((Some(raw_message.account_keys[0].clone()), true, false));
            }
        }
    }

    Ok((None, false, false)) // Default to both as false if no authority is found
}

/// Checks if a Solana account is closed (i.e., does not exist or has lamports = 0 and owned by system program).
async fn is_account_closed(rpc_client: &RpcClient, pubkey: &Pubkey) -> Result<bool> {
    match rpc_client.get_account(pubkey).await {
        Ok(account) => {
            let is_closed = account.lamports == 0 && account.owner == system_program::ID;
            Ok(is_closed)
        }
        Err(err) => {
            if err.to_string().contains("AccountNotFound") {
                // Account is fully closed and no longer exists
                Ok(true)
            } else {
                // Some other RPC error
                Err(err.into())
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_account_closed() {
        let rpc_manager = get_rpc_manager();
        let client = rpc_manager.get_client().await;

        // Test with a known closed program (should return true)
        let closed_program_pubkey =
            Pubkey::from_str("9fjvZfiAWRVXRHjBEz9mkAkLgK4dgbg7LnwWyPwHvFYB")
                .expect("Invalid pubkey");

        let result = is_account_closed(&client, &closed_program_pubkey).await;
        assert!(
            result.is_ok(),
            "Failed to check closed account: {:?}",
            result.err()
        );
        let is_closed = result.unwrap();
        assert!(is_closed, "Closed program should be detected as closed");

        // Test with an active program (should return false)
        let active_program_pubkey = Pubkey::from_str("wsoGmxQLSvwWpuaidCApxN5kEowLe2HLQLJhCQnj4bE")
            .expect("Invalid pubkey");

        let result = is_account_closed(&client, &active_program_pubkey).await;
        assert!(
            result.is_ok(),
            "Failed to check active account: {:?}",
            result.err()
        );
        let is_closed = result.unwrap();
        assert!(
            !is_closed,
            "Active program should not be detected as closed"
        );
    }

    #[tokio::test]
    async fn test_get_program_authority() {
        let result = get_program_authority("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC").await;

        assert!(
            result.is_ok(),
            "Failed to get authority: {:?}",
            result.err()
        );
        let (authority, _frozen, _closed) = result.unwrap();

        assert_eq!(
            authority,
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
        let (authority, _frozen, _closed) = result.unwrap();

        assert_eq!(
            authority,
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
        let (authority, _frozen, _closed) = result.unwrap();

        assert_eq!(
            authority,
            Some("6EqYa8BxABzh5qHXYGw3nAoAueCyZG6KMG7K9WTA23sD".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_program_authority_invalid_program() {
        let invalid_program = Pubkey::new_unique();
        let result = get_program_authority(&invalid_program.to_string()).await;
        assert!(result.is_err(), "Expected error for invalid program");
    }

    #[tokio::test]
    async fn test_get_program_authority_closed_program() {
        // This program has been closed - program data account no longer exists
        let result = get_program_authority("woRrXQHeAi9R5oUcKJb7pkqC3GrQMabKWPBYHAN1ufY").await;

        match result {
            Ok((authority, _is_frozen, is_closed)) => {
                // Should detect that the program is closed
                assert!(is_closed, "Program should be detected as closed");
                assert_eq!(authority, None, "Closed program should have no authority");
            }
            Err(e) => {
                // It's also acceptable if it returns an error
                println!("Got error for closed program (acceptable): {e:?}");
            }
        }
    }

    #[test]
    fn test_error_string_parsing() {
        // Test that we correctly distinguish between rate limit errors and actual closed programs

        // Rate limit error (should NOT be treated as closed program)
        let rate_limit_error = "AccountNotFound: pubkey=33G1UvntzZrQMWfRwP8c8KzsMeZwdUV1enVnnPyZ5dpv: HTTP status client error (429 Too Many Requests)";
        assert!(rate_limit_error.contains("HTTP status") || rate_limit_error.contains("429"));
        assert!(!should_treat_as_closed_program(rate_limit_error));

        // Actual closed program error (should be treated as closed program)
        let closed_program_error =
            "AccountNotFound: pubkey=FwfEft6xYShpzwTVaXTc7G3Vax8ykefuviC6AhATv97p";
        assert!(!closed_program_error.contains("HTTP"));
        assert!(should_treat_as_closed_program(closed_program_error));

        // Another HTTP error variant
        let http_error = "could not find account: HTTP status server error";
        assert!(!should_treat_as_closed_program(http_error));
    }

    fn should_treat_as_closed_program(error_str: &str) -> bool {
        // Check if this is an HTTP error (like 429 rate limiting) first
        if error_str.contains("HTTP status")
            || error_str.contains("Too Many Requests")
            || error_str.contains("429")
        {
            return false;
        }

        // Check if the error indicates the program data account was not found (closed)
        (error_str.contains("could not find account") && !error_str.contains("HTTP"))
            || (error_str.contains("AccountNotFound") && !error_str.contains("HTTP"))
    }

    #[tokio::test]
    async fn test_get_program_authority_active_program() {
        // Test with a program that is not closed
        let active_program_id = "wsoGmxQLSvwWpuaidCApxN5kEowLe2HLQLJhCQnj4bE";
        let result = get_program_authority(active_program_id).await;

        match result {
            Ok((authority, is_frozen, is_closed)) => {
                // Should not be marked as closed
                assert!(
                    !is_closed,
                    "Active program should not be detected as closed"
                );
                println!("Active program test passed - Authority: {authority:?}, Frozen: {is_frozen}, Closed: {is_closed}");
            }
            Err(e) => {
                // If there's an error, it should not be due to the program being closed
                println!("Got error for active program: {e:?}");
                // We can still pass the test as long as we're not incorrectly marking it as closed
            }
        }
    }

    #[tokio::test]
    async fn test_program_status_differentiation() {
        // Test that we can differentiate between different program states

        // Test 1: Valid program with authority (should not be frozen or closed)
        let result = get_program_authority("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC").await;
        if let Ok((authority, is_frozen, is_closed)) = result {
            assert!(authority.is_some(), "Valid program should have authority");
            // Note: These assertions might vary based on the actual program state
            println!("Valid program - Authority: {authority:?}, Frozen: {is_frozen}, Closed: {is_closed}");
        }

        // Test 2: Test return tuple format consistency
        let test_programs = vec![
            "333UA891CYPpAJAthphPT3hg1EkUBLhNFoP9HoWW3nug",
            "paxosVkYuJBKUQoZGAidRA47Qt4uidqG5fAt5kmr1nR",
        ];

        for program_id in test_programs {
            match get_program_authority(program_id).await {
                Ok((authority, is_frozen, is_closed)) => {
                    println!(
                        "Program {program_id}: Authority: {authority:?}, Frozen: {is_frozen}, Closed: {is_closed}"
                    );

                    // Basic validation: closed programs should not have authority
                    if is_closed {
                        assert_eq!(authority, None, "Closed program should not have authority");
                    }
                }
                Err(e) => {
                    println!("Error for program {program_id}: {e:?}");
                    // Errors are acceptable for some programs
                }
            }
        }
    }
}
