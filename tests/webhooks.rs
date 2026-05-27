//! Helius webhook endpoints (`/pda`, `/unverify`): auth, payload-shape,
//! and behavioural tests. Both handlers `tokio::spawn` their work and
//! return 200 before any DB or RPC effects land, so the behavioural
//! tests poll the DB via `wait_until`.

mod common;

use axum::http::StatusCode;
use common::rpc::{
    account_value, compute_program_hash, program_account_bytes, program_data_account_bytes,
    program_data_pda, MockRpc,
};
use common::{boot, boot_with_rpc, post, post_with_auth, wait_until, AUTH_SECRET};
use serde_json::json;
use solana_pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;
use std::time::Duration;
use verified_programs_api::onchain::{ProgramOnchainState, OTTER_VERIFY_PROGRAM_ID};
use verified_programs_api::types::Address;

const UPGRADE_DATA: &str = "5Sxr3";

// --- Auth-shape tests (don't need wiremock) -------------------------------

#[tokio::test]
async fn pda_without_auth_returns_401() {
    let (app, _pg) = boot().await;
    let (status, _) = post(app, "/pda", "[]").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn pda_with_wrong_auth_returns_401() {
    let (app, _pg) = boot().await;
    let (status, _) = post_with_auth(app, "/pda", "wrong", "[]").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn pda_with_correct_auth_but_empty_payload_returns_400() {
    let (app, _pg) = boot().await;
    let (status, _) = post_with_auth(app, "/pda", AUTH_SECRET, "[]").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unverify_without_auth_returns_401() {
    let (app, _pg) = boot().await;
    let (status, _) = post(app, "/unverify", "[]").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// --- /unverify behaviour ---------------------------------------------------

/// Builds a Helius "parsed transaction" array containing one upgrade
/// instruction whose `accounts[1]` is the given program id.
fn upgrade_payload(program_id: &Pubkey) -> String {
    let payload = json!([{
        "instructions": [{
            "accounts": ["sysvar", program_id.to_string(), "spill"],
            "data": UPGRADE_DATA,
            "programId": bpf_loader_upgradeable::ID.to_string(),
        }]
    }]);
    payload.to_string()
}

#[tokio::test]
async fn unverify_marks_closed_when_program_data_missing() {
    let rpc = MockRpc::start().await;
    let program_id = Pubkey::new_unique();

    // Program account doesn't exist -> snapshot sees None ->
    // get_on_chain_hash returns "Program appears to be closed" error ->
    // process_program_upgrade calls mark_closed.
    rpc.expect_get_multiple_accounts_for(&[program_id], vec![None])
        .await;

    let (app, db, _pg) = boot_with_rpc(&rpc.uri()).await;
    let (status, _) =
        post_with_auth(app, "/unverify", AUTH_SECRET, &upgrade_payload(&program_id)).await;
    assert_eq!(status, StatusCode::OK);

    let addr = Address(program_id);
    let ok = wait_until(Duration::from_secs(5), || async {
        matches!(db.get_program_state(&addr).await, Ok(Some(s)) if s.is_closed)
    })
    .await;
    assert!(ok, "program_state.is_closed should flip to true");
}

#[tokio::test]
async fn unverify_updates_hash_when_program_upgraded() {
    let rpc = MockRpc::start().await;
    let program_id = Pubkey::new_unique();
    let pda = program_data_pda(&program_id);
    let authority = Pubkey::new_unique();
    let new_bytecode = b"new program bytes after upgrade";
    let new_hash = compute_program_hash(new_bytecode);

    rpc.expect_get_multiple_accounts_for(
        &[program_id],
        vec![Some(account_value(
            &bpf_loader_upgradeable::ID,
            &program_account_bytes(&pda),
            1_000_000,
        ))],
    )
    .await;
    rpc.expect_get_multiple_accounts_for(
        &[pda],
        vec![Some(account_value(
            &bpf_loader_upgradeable::ID,
            &program_data_account_bytes(0, Some(&authority), new_bytecode),
            1_000_000,
        ))],
    )
    .await;

    let (app, db, _pg) = boot_with_rpc(&rpc.uri()).await;
    let addr = Address(program_id);
    db.upsert_program_state(
        &addr,
        &ProgramOnchainState {
            authority: None,
            is_frozen: false,
            is_closed: false,
            executable_hash: Some("old_hash".to_string()),
        },
    )
    .await
    .unwrap();

    let (status, _) =
        post_with_auth(app, "/unverify", AUTH_SECRET, &upgrade_payload(&program_id)).await;
    assert_eq!(status, StatusCode::OK);

    let ok = wait_until(Duration::from_secs(5), || async {
        db.cached_on_chain_hash(&addr).await.ok().as_deref() == Some(new_hash.as_str())
    })
    .await;
    assert!(ok, "on_chain_hash should be updated to {new_hash}");
}

#[tokio::test]
async fn unverify_noop_when_hash_unchanged() {
    let rpc = MockRpc::start().await;
    let program_id = Pubkey::new_unique();
    let pda = program_data_pda(&program_id);
    let authority = Pubkey::new_unique();
    let bytecode = b"unchanged program bytes";
    let hash = compute_program_hash(bytecode);

    rpc.expect_get_multiple_accounts_for(
        &[program_id],
        vec![Some(account_value(
            &bpf_loader_upgradeable::ID,
            &program_account_bytes(&pda),
            1_000_000,
        ))],
    )
    .await;
    rpc.expect_get_multiple_accounts_for(
        &[pda],
        vec![Some(account_value(
            &bpf_loader_upgradeable::ID,
            &program_data_account_bytes(0, Some(&authority), bytecode),
            1_000_000,
        ))],
    )
    .await;

    let (app, db, _pg) = boot_with_rpc(&rpc.uri()).await;
    let addr = Address(program_id);
    db.upsert_program_state(
        &addr,
        &ProgramOnchainState {
            authority: None,
            is_frozen: false,
            is_closed: false,
            executable_hash: Some(hash.clone()),
        },
    )
    .await
    .unwrap();
    let original = db.get_program_state(&addr).await.unwrap().unwrap();

    let (status, _) =
        post_with_auth(app, "/unverify", AUTH_SECRET, &upgrade_payload(&program_id)).await;
    assert_eq!(status, StatusCode::OK);

    // Wait long enough for the spawned task to have run, then assert
    // nothing changed.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let after = db.get_program_state(&addr).await.unwrap().unwrap();
    assert_eq!(after.on_chain_hash, original.on_chain_hash);
    assert_eq!(after.is_closed, original.is_closed);
}

#[tokio::test]
async fn unverify_ignores_non_upgrade_instructions() {
    let rpc = MockRpc::start().await;
    let program_id = Pubkey::new_unique();
    let (app, db, _pg) = boot_with_rpc(&rpc.uri()).await;

    let payload = json!([{
        "instructions": [{
            "accounts": ["sysvar", program_id.to_string(), "spill"],
            "data": "NotAnUpgrade",
            "programId": bpf_loader_upgradeable::ID.to_string(),
        }]
    }])
    .to_string();

    let (status, _) = post_with_auth(app, "/unverify", AUTH_SECRET, &payload).await;
    assert_eq!(status, StatusCode::OK);

    // Give the spawned task a beat to finish; if it touched RPC or
    // wrote to program_state, we'd see it.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let state = db.get_program_state(&Address(program_id)).await.unwrap();
    assert!(
        state.is_none(),
        "non-upgrade instructions must not trigger any DB writes"
    );
    assert_eq!(
        rpc.method_call_count("getMultipleAccounts").await,
        0,
        "non-upgrade instructions must not hit RPC"
    );
}

// --- /pda behaviour --------------------------------------------------------

/// Builds a /pda payload with one Otter Verify instruction:
///   accounts[0] = pda, accounts[2] = program_id.
#[allow(dead_code)] // wired up once the ignored /pda tests un-ignore
fn pda_payload(pda: &Pubkey, program_id: &Pubkey) -> String {
    json!([{
        "instructions": [{
            "accounts": [pda.to_string(), "filler", program_id.to_string()],
            "data": "",
            "programId": OTTER_VERIFY_PROGRAM_ID.to_string(),
        }]
    }])
    .to_string()
}

#[tokio::test]
async fn pda_ignores_unknown_pda() {
    // "unknown" here means: the instruction's programId isn't Otter
    // Verify, so the handler's filter skips it entirely and never
    // touches RPC or the DB.
    let rpc = MockRpc::start().await;
    let program_id = Pubkey::new_unique();
    let pda = Pubkey::new_unique();
    let (app, db, _pg) = boot_with_rpc(&rpc.uri()).await;

    let payload = json!([{
        "instructions": [{
            "accounts": [pda.to_string(), "filler", program_id.to_string()],
            "data": "",
            "programId": bpf_loader_upgradeable::ID.to_string(),  // NOT Otter Verify
        }]
    }])
    .to_string();

    let (status, _) = post_with_auth(app, "/pda", AUTH_SECRET, &payload).await;
    assert_eq!(status, StatusCode::OK);

    tokio::time::sleep(Duration::from_millis(300)).await;
    let state = db.get_program_state(&Address(program_id)).await.unwrap();
    assert!(state.is_none(), "non-Otter ix must not write state");
    assert_eq!(
        rpc.method_call_count("getMultipleAccounts").await,
        0,
        "non-Otter ix must not hit RPC"
    );
}

#[tokio::test]
#[ignore = "spawns process_verification which shells out to solana-verify; verify smoke covers this end-to-end"]
async fn pda_enqueues_build_for_valid_otter_pda() {
    // Mocking the full flow (snapshot_programs x N, get_account_data on
    // PDA, then setup_verification's RPC calls) is doable but invasive.
    // The smoke test in `verify_smoke.rs` covers the full chain end-to-end
    // against real RPC; leaving this ignored until we either move
    // `process_verification`'s build spawn behind a trait or accept the
    // mocking complexity.
}

#[tokio::test]
#[ignore = "same RPC-mocking complexity as pda_enqueues_build_for_valid_otter_pda"]
async fn pda_dedupes_existing_inprogress_build() {
    // Pre-seeding the duplicate requires constructing a NewBuild whose
    // fields match what `NewBuild::from(&OtterBuildParams)` would
    // produce given a mocked Otter Verify PDA. That ties the test to
    // internal layout in a brittle way; defer until the build-spawn
    // path is testable in isolation.
}
