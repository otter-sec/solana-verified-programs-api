//! Helius webhook endpoints (`/pda`, `/unverify`): auth, payload-shape,
//! and behavioural tests. Both handlers `tokio::spawn` their work and
//! return 200 before any DB or RPC effects land, so the behavioural
//! tests poll the DB via `wait_until`.

mod common;

use axum::http::StatusCode;
use common::rpc::{
    account_value, compute_program_hash, encode_otter_pda, program_account_bytes,
    program_data_account_bytes, program_data_pda, MockRpc,
};
use common::{boot, boot_with_rpc, post, post_with_auth, wait_until, AUTH_SECRET};
use serde_json::json;
use solana_pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;
use std::time::Duration;
use verified_programs_api::db::{DbClient, NewBuild};
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

/// `/unverify` only acts on programs we've verified before (those with a
/// completed build); otherwise it skips them. Seed a completed build so the
/// program under test counts as tracked.
async fn seed_tracked_program(db: &DbClient, program_id: &Address) {
    let build_id = db
        .insert_build(&NewBuild {
            repository: "https://github.com/x/y".to_string(),
            commit_hash: Some("deadbeef".to_string()),
            program_id: *program_id,
            lib_name: None,
            base_docker_image: None,
            mount_path: None,
            cargo_args: None,
            cargo_build_sbf_args: None,
            bpf_flag: false,
            arch: None,
            signer: None,
        })
        .await
        .expect("insert build");
    db.mark_build_completed(build_id, program_id, "seeded_hash")
        .await
        .expect("complete build");
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
    let addr = Address(program_id);
    seed_tracked_program(&db, &addr).await;

    let (status, _) =
        post_with_auth(app, "/unverify", AUTH_SECRET, &upgrade_payload(&program_id)).await;
    assert_eq!(status, StatusCode::OK);

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
    seed_tracked_program(&db, &addr).await;

    let (status, _) =
        post_with_auth(app, "/unverify", AUTH_SECRET, &upgrade_payload(&program_id)).await;
    assert_eq!(status, StatusCode::OK);

    let ok = wait_until(Duration::from_secs(5), || async {
        db.cached_on_chain_hash(&addr)
            .await
            .ok()
            .flatten()
            .as_deref()
            == Some(new_hash.as_str())
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
async fn unverify_skips_unverified_program() {
    let rpc = MockRpc::start().await;
    let program_id = Pubkey::new_unique();
    let (app, db, _pg) = boot_with_rpc(&rpc.uri()).await;
    let addr = Address(program_id);

    // No completed build -> a program we never verified, so the handler must
    // skip it: no RPC lookup, and no junk program_state row created.
    let (status, _) =
        post_with_auth(app, "/unverify", AUTH_SECRET, &upgrade_payload(&program_id)).await;
    assert_eq!(status, StatusCode::OK);

    // Give the spawned task a beat to run, then assert it did nothing.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        db.get_program_state(&addr).await.unwrap().is_none(),
        "unknown program must not get a program_state row"
    );
    assert_eq!(
        rpc.request_count().await,
        0,
        "handler should skip before making any RPC call"
    );
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

/// Constants shared by the two /pda behavioural tests below.
const PDA_TEST_REPO: &str = "https://github.com/test/program";
const PDA_TEST_COMMIT: &str = "deadbeef";

/// Sets up the full mock chain that `pda_worker` walks when an Otter
/// Verify PDA event arrives for a program whose on-chain hash has
/// drifted. Returns the program id (as `Address`), the PDA account
/// pubkey, and the Otter PDA signer.
async fn mock_pda_event(rpc: &MockRpc, bytecode: &[u8]) -> (Address, Pubkey, Address) {
    let program_id = Pubkey::new_unique();
    let program_data = program_data_pda(&program_id);
    let authority = Pubkey::new_unique();
    let signer = Address(Pubkey::new_unique());
    // The PDA account address in the webhook ix is opaque to the handler
    // (it just calls get_account_data on it), so any pubkey works.
    let pda_account = Pubkey::new_unique();

    // get_on_chain_hash -> snapshot_programs(program, program_data)
    rpc.expect_get_multiple_accounts_for(
        &[program_id],
        vec![Some(account_value(
            &bpf_loader_upgradeable::ID,
            &program_account_bytes(&program_data),
            1_000_000,
        ))],
    )
    .await;
    rpc.expect_get_multiple_accounts_for(
        &[program_data],
        vec![Some(account_value(
            &bpf_loader_upgradeable::ID,
            &program_data_account_bytes(0, Some(&authority), bytecode),
            1_000_000,
        ))],
    )
    .await;

    // get_account_data(pda_account) -> the Borsh-encoded Otter params.
    let otter_bytes = encode_otter_pda(
        &program_id,
        signer.as_pubkey(),
        "",
        PDA_TEST_REPO,
        PDA_TEST_COMMIT,
        &[],
        0,
    );
    rpc.expect_get_account_info_for(
        &pda_account,
        Some(account_value(
            &OTTER_VERIFY_PROGRAM_ID,
            &otter_bytes,
            1_000_000,
        )),
    )
    .await;

    (Address(program_id), pda_account, signer)
}

#[tokio::test]
async fn pda_enqueues_build_for_valid_otter_pda() {
    let rpc = MockRpc::start().await;
    let (program_id, pda_account, signer) = mock_pda_event(&rpc, b"fresh bytecode v2").await;

    let (app, db, _pg) = boot_with_rpc(&rpc.uri()).await;

    let payload = json!([{
        "instructions": [{
            "accounts": [pda_account.to_string(), "filler", program_id.to_string()],
            "data": "",
            "programId": OTTER_VERIFY_PROGRAM_ID.to_string(),
        }]
    }])
    .to_string();

    let (status, _) = post_with_auth(app, "/pda", AUTH_SECRET, &payload).await;
    assert_eq!(status, StatusCode::OK);

    // pda_worker -> process_verification -> create_and_insert_build
    // inserts the row synchronously; the build run is spawned and will
    // fail (no Docker) but the row is already there.
    let ok = wait_until(Duration::from_secs(5), || async {
        let builds: Vec<(uuid::Uuid,)> =
            sqlx::query_as("SELECT id FROM builds WHERE program_id = $1 AND repository = $2")
                .bind(program_id)
                .bind(PDA_TEST_REPO)
                .fetch_all(db.pool())
                .await
                .unwrap_or_default();
        !builds.is_empty()
    })
    .await;
    assert!(ok, "expected a build row to be enqueued for the program");

    // The inserted row carries the signer from the Otter PDA, not None.
    let signer_col: Option<String> =
        sqlx::query_scalar("SELECT signer FROM builds WHERE program_id = $1 AND repository = $2")
            .bind(program_id)
            .bind(PDA_TEST_REPO)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(signer_col.as_deref(), Some(signer.to_string().as_str()));
}

#[tokio::test]
async fn pda_dedupes_existing_inprogress_build() {
    let rpc = MockRpc::start().await;
    let (program_id, pda_account, signer) = mock_pda_event(&rpc, b"another bytecode rev").await;

    let (app, db, _pg) = boot_with_rpc(&rpc.uri()).await;

    // Pre-seed a build row with exactly the fields `NewBuild::from(&OtterBuildParams)`
    // will produce, in `in_progress`. find_duplicate must match it and
    // skip the insert.
    db.insert_build(&NewBuild {
        repository: PDA_TEST_REPO.to_string(),
        commit_hash: Some(PDA_TEST_COMMIT.to_string()),
        program_id,
        lib_name: None,
        base_docker_image: None,
        mount_path: None,
        cargo_args: None,
        cargo_build_sbf_args: None,
        bpf_flag: false,
        arch: None,
        signer: Some(signer),
    })
    .await
    .unwrap();

    let payload = json!([{
        "instructions": [{
            "accounts": [pda_account.to_string(), "filler", program_id.to_string()],
            "data": "",
            "programId": OTTER_VERIFY_PROGRAM_ID.to_string(),
        }]
    }])
    .to_string();

    let (status, _) = post_with_auth(app, "/pda", AUTH_SECRET, &payload).await;
    assert_eq!(status, StatusCode::OK);

    // unverify_program (which precedes find_duplicate) fires either way;
    // poll for its on-chain hash to land, then assert build count stayed
    // at 1.
    let ok = wait_until(Duration::from_secs(5), || async {
        db.cached_on_chain_hash(&program_id)
            .await
            .ok()
            .flatten()
            .is_some_and(|h| !h.is_empty())
    })
    .await;
    assert!(ok, "expected unverify_program to update on_chain_hash");

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM builds WHERE program_id = $1")
        .bind(program_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count.0, 1, "duplicate build must not be inserted");
}
