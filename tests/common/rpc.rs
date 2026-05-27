//! Wiremock-backed Solana JSON-RPC harness for tests that exercise
//! code paths reaching the RPC layer (`get_multiple_accounts`,
//! `get_account_info`, `get_account_data`).
//!
//! Matchers key on the JSON-RPC `method` field, so the order of
//! registration doesn't matter across distinct methods. Within a single
//! method, the most-recently-mounted mock wins -- handy for re-arming
//! a method between calls in a single test.

#![allow(dead_code)]

use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;
use wiremock::matchers::{body_partial_json, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub struct MockRpc {
    pub server: MockServer,
}

impl MockRpc {
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    pub fn uri(&self) -> String {
        self.server.uri()
    }

    /// Stand-in for `getMultipleAccounts`. `values` lines up with the
    /// request's pubkey order; `None` means "account doesn't exist".
    pub async fn expect_get_multiple_accounts(&self, values: Vec<Option<Value>>) {
        Mock::given(method("POST"))
            .and(body_partial_json(json!({"method": "getMultipleAccounts"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "context": {"apiVersion": "2.0.0", "slot": 0},
                    "value": values,
                }
            })))
            .mount(&self.server)
            .await;
    }

    /// Like [`expect_get_multiple_accounts`] but only matches calls
    /// whose `params[0]` array contains exactly `pubkeys` (in order).
    /// Use this to discriminate between the two `getMultipleAccounts`
    /// calls made by `snapshot_programs` (one for program accounts, one
    /// for program-data PDAs).
    pub async fn expect_get_multiple_accounts_for(
        &self,
        pubkeys: &[Pubkey],
        values: Vec<Option<Value>>,
    ) {
        let pubkey_strs: Vec<String> = pubkeys.iter().map(|p| p.to_string()).collect();
        Mock::given(method("POST"))
            .and(body_partial_json(json!({
                "method": "getMultipleAccounts",
                "params": [pubkey_strs],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "context": {"apiVersion": "2.0.0", "slot": 0},
                    "value": values,
                }
            })))
            .mount(&self.server)
            .await;
    }

    /// Stand-in for `getAccountInfo`. `account` is the value object
    /// (built with [`account_value`]), or `None` for a missing account.
    pub async fn expect_get_account_info(&self, account: Option<Value>) {
        Mock::given(method("POST"))
            .and(body_partial_json(json!({"method": "getAccountInfo"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "context": {"apiVersion": "2.0.0", "slot": 0},
                    "value": account,
                }
            })))
            .mount(&self.server)
            .await;
    }

    /// Number of HTTP requests the mock server has received so far.
    pub async fn request_count(&self) -> usize {
        self.server
            .received_requests()
            .await
            .map(|r| r.len())
            .unwrap_or(0)
    }

    /// Count requests whose body's `method` field equals `name`. Useful
    /// for asserting things like "we made exactly N getMultipleAccounts
    /// calls".
    pub async fn method_call_count(&self, name: &str) -> usize {
        self.server
            .received_requests()
            .await
            .map(|reqs| {
                reqs.iter()
                    .filter(|r| {
                        serde_json::from_slice::<Value>(&r.body)
                            .ok()
                            .and_then(|v| {
                                v.get("method").and_then(|m| m.as_str()).map(String::from)
                            })
                            .as_deref()
                            == Some(name)
                    })
                    .count()
            })
            .unwrap_or(0)
    }
}

/// A single account-info value as the RPC client expects, with `data`
/// base64-encoded.
pub fn account_value(owner: &Pubkey, data: &[u8], lamports: u64) -> Value {
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    json!({
        "data": [encoded, "base64"],
        "executable": false,
        "lamports": lamports,
        "owner": owner.to_string(),
        "rentEpoch": 18446744073709551615u64,
        "space": data.len(),
    })
}

/// Bincode-serialized `UpgradeableLoaderState::Program { programdata_address }`.
/// 4-byte enum tag (2) + 32-byte pubkey = 36 bytes.
pub fn program_account_bytes(program_data_pda: &Pubkey) -> Vec<u8> {
    let mut out = Vec::with_capacity(36);
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&program_data_pda.to_bytes());
    out
}

/// Bincode-serialized `UpgradeableLoaderState::ProgramData { slot, authority }`
/// followed by the executable bytecode. 4 + 8 + 1 + 32 byte header (45)
/// when authority is `Some`, plus `bytecode`.
pub fn program_data_account_bytes(
    slot: u64,
    authority: Option<&Pubkey>,
    bytecode: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(45 + bytecode.len());
    out.extend_from_slice(&3u32.to_le_bytes());
    out.extend_from_slice(&slot.to_le_bytes());
    match authority {
        Some(a) => {
            out.push(1);
            out.extend_from_slice(&a.to_bytes());
        }
        None => out.push(0),
    }
    out.extend_from_slice(bytecode);
    out
}

/// Mirror of `onchain::state::compute_program_hash`: sha256 of the
/// bytes with trailing zeros stripped, hex-encoded. Tests use this to
/// predict what the sweep will write to `program_state.on_chain_hash`.
pub fn compute_program_hash(data: &[u8]) -> String {
    let trimmed = match data.iter().rposition(|&b| b != 0) {
        Some(i) => &data[..=i],
        None => &[][..],
    };
    let mut hasher = Sha256::new();
    hasher.update(trimmed);
    hex::encode(hasher.finalize())
}

/// Derives the program-data PDA for a given program (mirrors what the
/// BPF loader does).
pub fn program_data_pda(program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::id()).0
}
