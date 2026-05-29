//! Program authority + on-chain snapshot retrieval.
//!
//! `snapshot_programs` is the batched (getMultipleAccounts) workhorse used
//! by the sweep; `get_program_state` is the single-program wrapper with
//! Squads/burned-authority recovery via transaction history.

use crate::errors::{ApiError, Result};
use sha2::{Digest, Sha256};
use solana_account_decoder::parse_bpf_loader::{
    parse_bpf_upgradeable_loader, BpfUpgradeableLoaderAccountType, UiProgram, UiProgramData,
};
use solana_client::{
    nonblocking::rpc_client::RpcClient, rpc_client::GetConfirmedSignaturesForAddress2Config,
    rpc_config::RpcTransactionConfig,
};
use solana_pubkey::Pubkey;
use solana_sdk_ids::{bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable};
use solana_signature::Signature;
use solana_transaction_status::{EncodedTransaction, UiMessage, UiTransactionEncoding};
use std::{collections::HashMap, str::FromStr};
use tracing::warn;

// Stable per the BPF loader v3 spec:
// bincode(UpgradeableLoaderState::ProgramData{slot, Some(Pubkey)}) =
//   4-byte enum tag + 8-byte slot + 1-byte option discriminator + 32-byte pubkey.
pub(super) const PROGRAM_DATA_HEADER_SIZE: usize = 45;

// Solana RPC servers cap getMultipleAccounts at 100.
const GMA_CHUNK: usize = 100;

// Squads-frozen programs route the final upgrade through the Squads multisig;
// the burned authority is the 5th account on this specific instruction.
const SQUADS_PROGRAM_ID: &str = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";
const SQUADS_AUTHORITY_IX_DATA: &str = "ZTNTtVtnvbC";
const SQUADS_AUTHORITY_ACCOUNT_INDEX: usize = 4;

/// Snapshot of the on-chain side of a program -- what gets written to `program_state`.
#[derive(Debug, Clone)]
pub struct ProgramOnchainState {
    pub authority: Option<String>,
    pub is_frozen: bool,
    pub is_closed: bool,
    pub executable_hash: Option<String>,
}

impl ProgramOnchainState {
    fn closed() -> Self {
        Self {
            authority: None,
            is_frozen: false,
            is_closed: true,
            executable_hash: None,
        }
    }

    fn empty() -> Self {
        Self {
            authority: None,
            is_frozen: false,
            is_closed: false,
            executable_hash: None,
        }
    }
}

/// Batched on-chain snapshot for many programs at once.
///
/// Two `getMultipleAccounts` calls per chunk of 100: one for the program
/// accounts (to extract program-data PDAs and handle legacy loaders), one
/// for the program-data accounts. Executable hash is computed inline from
/// the bytes -- no `solana-verify` subprocess.
///
/// For programs frozen with no authority on the program-data account,
/// `authority` is left `None` and `is_frozen` is `true`. The Squads
/// transaction-history recovery only runs from [`get_program_state`].
pub async fn snapshot_programs(
    rpc: &RpcClient,
    ids: &[Pubkey],
) -> Result<HashMap<Pubkey, ProgramOnchainState>> {
    let mut out = HashMap::with_capacity(ids.len());
    for chunk in ids.chunks(GMA_CHUNK) {
        snapshot_chunk(rpc, chunk, &mut out).await?;
    }
    Ok(out)
}

async fn snapshot_chunk(
    rpc: &RpcClient,
    ids: &[Pubkey],
    out: &mut HashMap<Pubkey, ProgramOnchainState>,
) -> Result<()> {
    let accounts = rpc
        .get_multiple_accounts(ids)
        .await
        .map_err(|e| ApiError::Custom(format!("getMultipleAccounts: {e}")))?;

    let mut to_fetch: Vec<(Pubkey, Pubkey)> = Vec::new();
    for (id, maybe_acc) in ids.iter().zip(accounts.into_iter()) {
        let Some(acc) = maybe_acc else {
            out.insert(*id, ProgramOnchainState::closed());
            continue;
        };
        if acc.owner == bpf_loader_upgradeable::ID {
            match extract_program_data_pda(&acc.data) {
                Ok(pda) => to_fetch.push((*id, pda)),
                Err(e) => {
                    warn!("program {} unparseable: {}", id, e);
                    out.insert(*id, ProgramOnchainState::closed());
                }
            }
        } else if acc.owner == bpf_loader::ID || acc.owner == bpf_loader_deprecated::ID {
            // Legacy loaders: the account data IS the executable; immutable.
            out.insert(
                *id,
                ProgramOnchainState {
                    authority: None,
                    is_frozen: true,
                    is_closed: false,
                    executable_hash: Some(compute_program_hash(&acc.data)),
                },
            );
        } else {
            warn!("program {} has unsupported owner {}", id, acc.owner);
            out.insert(*id, ProgramOnchainState::closed());
        }
    }

    if to_fetch.is_empty() {
        return Ok(());
    }

    let pdas: Vec<Pubkey> = to_fetch.iter().map(|(_, p)| *p).collect();
    let pda_accounts = rpc
        .get_multiple_accounts(&pdas)
        .await
        .map_err(|e| ApiError::Custom(format!("getMultipleAccounts(program_data): {e}")))?;

    for ((program_id, _), maybe_acc) in to_fetch.iter().zip(pda_accounts.into_iter()) {
        match maybe_acc {
            None => {
                out.insert(*program_id, ProgramOnchainState::closed());
            }
            Some(acc) => {
                // If we can't parse the program-data account at all (corrupt
                // bytes, unexpected loader account type), skip this program
                // rather than overwriting its cached state with a guess.
                let authority = match parse_program_data_authority(&acc.data) {
                    Ok(a) => a,
                    Err(e) => {
                        warn!("program {} program_data unparseable: {}", program_id, e);
                        continue;
                    }
                };
                let hash = if acc.data.len() >= PROGRAM_DATA_HEADER_SIZE {
                    Some(compute_program_hash(&acc.data[PROGRAM_DATA_HEADER_SIZE..]))
                } else {
                    None
                };
                out.insert(
                    *program_id,
                    ProgramOnchainState {
                        is_frozen: authority.is_none(),
                        authority,
                        is_closed: false,
                        executable_hash: hash,
                    },
                );
            }
        }
    }
    Ok(())
}

/// `Ok(Some(_))` -- has authority. `Ok(None)` -- frozen (no authority).
/// `Err(_)` -- parse failure, caller should not interpret either way.
fn parse_program_data_authority(data: &[u8]) -> Result<Option<String>> {
    match parse_bpf_upgradeable_loader(data)? {
        BpfUpgradeableLoaderAccountType::ProgramData(UiProgramData { authority, .. }) => {
            Ok(authority)
        }
        other => Err(ApiError::Custom(format!(
            "expected ProgramData account, got: {other:?}"
        ))),
    }
}

/// Single-program snapshot, with Squads/burned-authority recovery via tx
/// history when the program looks frozen but has no on-chain authority.
/// Used by the verify path where the authority drives Otter Verify PDA lookup.
pub async fn get_program_state(
    rpc: &RpcClient,
    program_id: &Pubkey,
) -> Result<ProgramOnchainState> {
    let mut state = snapshot_programs(rpc, &[*program_id])
        .await?
        .remove(program_id)
        .unwrap_or_else(ProgramOnchainState::empty);
    if state.is_frozen && state.authority.is_none() && !state.is_closed {
        if let Ok(Some(auth)) = recover_burned_authority(rpc, program_id).await {
            state.authority = Some(auth);
        }
    }
    Ok(state)
}

async fn recover_burned_authority(rpc: &RpcClient, program_id: &Pubkey) -> Result<Option<String>> {
    let program_data_pda =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::id()).0;
    let cfg = GetConfirmedSignaturesForAddress2Config {
        limit: Some(1),
        before: None,
        until: None,
        commitment: None,
    };
    let sigs = rpc
        .get_signatures_for_address_with_config(&program_data_pda, cfg)
        .await
        .map_err(|e| ApiError::Custom(e.to_string()))?;
    let Some(latest) = sigs.first() else {
        return Ok(None);
    };
    let sig = Signature::from_str(&latest.signature)
        .map_err(|e| ApiError::Custom(format!("parse signature: {e}")))?;
    let tx = rpc
        .get_transaction_with_config(
            &sig,
            RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Json),
                commitment: None,
                max_supported_transaction_version: Some(0),
            },
        )
        .await?;
    if let EncodedTransaction::Json(ui) = tx.transaction.transaction {
        if let UiMessage::Raw(raw) = &ui.message {
            if let Some(squads_idx) = raw.account_keys.iter().position(|k| k == SQUADS_PROGRAM_ID) {
                let squads_idx = squads_idx as u8;
                for ix in &raw.instructions {
                    if ix.program_id_index == squads_idx && ix.data == SQUADS_AUTHORITY_IX_DATA {
                        let aidx = ix.accounts[SQUADS_AUTHORITY_ACCOUNT_INDEX] as usize;
                        return Ok(Some(raw.account_keys[aidx].clone()));
                    }
                }
            }
            return Ok(Some(raw.account_keys[0].clone()));
        }
    }
    Ok(None)
}

fn extract_program_data_pda(data: &[u8]) -> Result<Pubkey> {
    match parse_bpf_upgradeable_loader(data)? {
        BpfUpgradeableLoaderAccountType::Program(UiProgram { program_data }) => {
            Pubkey::from_str(&program_data).map_err(Into::into)
        }
        other => Err(ApiError::Custom(format!(
            "expected Program account, got: {other:?}"
        ))),
    }
}

/// `sha256(data with trailing zeros stripped)`, hex-encoded. Matches
/// `solana-verify get-program-hash`'s output byte-for-byte.
//
// TODO: solana-verify is binary-only today. If it ships a library crate
// we can drop this function and the PROGRAM_DATA_HEADER_SIZE constant in
// favour of their `get_binary_hash` /
// `UpgradeableLoaderState::size_of_programdata_metadata()`.
fn compute_program_hash(data: &[u8]) -> String {
    let trimmed = match data.iter().rposition(|&b| b != 0) {
        Some(i) => &data[..=i],
        None => &[][..],
    };
    let mut hasher = Sha256::new();
    hasher.update(trimmed);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rpc() -> RpcClient {
        RpcClient::new("https://api.mainnet-beta.solana.com".to_string())
    }

    #[tokio::test]
    #[ignore = "hits mainnet RPC"]
    async fn test_get_program_state_active() {
        let pid = Pubkey::from_str("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC").unwrap();
        let state = get_program_state(&rpc(), &pid).await.expect("state");
        assert!(!state.is_closed);
        assert_eq!(
            state.authority.as_deref(),
            Some("9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU")
        );
    }

    #[tokio::test]
    #[ignore = "hits mainnet RPC"]
    async fn test_get_program_state_frozen() {
        let pid = Pubkey::from_str("333UA891CYPpAJAthphPT3hg1EkUBLhNFoP9HoWW3nug").unwrap();
        let state = get_program_state(&rpc(), &pid).await.expect("state");
        assert!(state.is_frozen);
        assert_eq!(
            state.authority.as_deref(),
            Some("FHKkBao61GZt3bkKbfMmd4GmDqQyYudyWQc5RUk4PKuZ")
        );
    }

    #[tokio::test]
    #[ignore = "hits mainnet RPC"]
    async fn test_get_program_state_squads_frozen() {
        let pid = Pubkey::from_str("paxosVkYuJBKUQoZGAidRA47Qt4uidqG5fAt5kmr1nR").unwrap();
        let state = get_program_state(&rpc(), &pid).await.expect("state");
        assert!(state.is_frozen);
        assert_eq!(
            state.authority.as_deref(),
            Some("6EqYa8BxABzh5qHXYGw3nAoAueCyZG6KMG7K9WTA23sD")
        );
    }

    #[tokio::test]
    #[ignore = "hits mainnet RPC"]
    async fn test_get_program_state_closed() {
        let pid = Pubkey::from_str("woRrXQHeAi9R5oUcKJb7pkqC3GrQMabKWPBYHAN1ufY").unwrap();
        let state = get_program_state(&rpc(), &pid).await.expect("state");
        assert!(state.is_closed);
        assert!(state.authority.is_none());
    }

    #[test]
    fn hash_strips_trailing_zeros() {
        // Empty payload (all zeros) hashes the empty string.
        let h = compute_program_hash(&[0u8; 16]);
        assert_eq!(
            h,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hash_known_bytes() {
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        let mut data = b"hello".to_vec();
        data.extend_from_slice(&[0u8; 8]);
        assert_eq!(
            compute_program_hash(&data),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
