//! Sweep behaviour: drive `sweep::run_once` directly against a mocked
//! Solana JSON-RPC and assert what lands in `program_state`. The
//! production sweep loop in `sweep::spawn` is just a ticker around
//! `run_once`, so testing the per-cycle function is enough.

mod common;

use common::pg_for_test;
use common::rpc::{
    account_value, compute_program_hash, program_account_bytes, program_data_account_bytes,
    program_data_pda, MockRpc,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;
use verified_programs_api::db::{DbClient, NewBuild};
use verified_programs_api::onchain::ProgramOnchainState;
use verified_programs_api::sweep;
use verified_programs_api::types::Address;

/// Build the DB + RPC client pair the sweep needs.
async fn boot_for_sweep() -> (
    DbClient,
    RpcClient,
    MockRpc,
    Option<
        testcontainers_modules::testcontainers::ContainerAsync<
            testcontainers_modules::postgres::Postgres,
        >,
    >,
) {
    let (url, pg) = pg_for_test().await;
    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("db connect");
    db.migrate().await.expect("migrate");
    let rpc_mock = MockRpc::start().await;
    let rpc = RpcClient::new(rpc_mock.uri());
    (db, rpc, rpc_mock, pg)
}

#[tokio::test]
async fn sweep_refreshes_open_program() {
    let (db, rpc, rpc_mock, _pg) = boot_for_sweep().await;
    let program_id = Pubkey::new_unique();
    let pda = program_data_pda(&program_id);
    let authority = Pubkey::new_unique();
    let bytecode = b"compiled program bytecode";
    let expected_hash = compute_program_hash(bytecode);

    let addr = Address(program_id);
    db.upsert_program_state(
        &addr,
        &ProgramOnchainState {
            authority: None,
            is_frozen: false,
            is_closed: false,
            executable_hash: None,
        },
    )
    .await
    .unwrap();

    rpc_mock
        .expect_get_multiple_accounts_for(
            &[program_id],
            vec![Some(account_value(
                &bpf_loader_upgradeable::ID,
                &program_account_bytes(&pda),
                1_000_000,
            ))],
        )
        .await;
    rpc_mock
        .expect_get_multiple_accounts_for(
            &[pda],
            vec![Some(account_value(
                &bpf_loader_upgradeable::ID,
                &program_data_account_bytes(0, Some(&authority), bytecode),
                1_000_000,
            ))],
        )
        .await;

    sweep::run_once(&db, &rpc).await.expect("sweep");

    let state = db.get_program_state(&addr).await.unwrap().unwrap();
    assert_eq!(state.on_chain_hash.as_deref(), Some(expected_hash.as_str()));
    assert!(!state.is_closed);
    assert!(!state.is_frozen);
}

#[tokio::test]
async fn sweep_marks_program_closed() {
    let (db, rpc, rpc_mock, _pg) = boot_for_sweep().await;
    let program_id = Pubkey::new_unique();
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

    rpc_mock
        .expect_get_multiple_accounts_for(&[program_id], vec![None])
        .await;

    sweep::run_once(&db, &rpc).await.expect("sweep");

    let state = db.get_program_state(&addr).await.unwrap().unwrap();
    assert!(state.is_closed);
}

#[tokio::test]
async fn sweep_marks_program_frozen() {
    let (db, rpc, rpc_mock, _pg) = boot_for_sweep().await;
    let program_id = Pubkey::new_unique();
    let pda = program_data_pda(&program_id);
    let addr = Address(program_id);

    db.upsert_program_state(
        &addr,
        &ProgramOnchainState {
            authority: None,
            is_frozen: false,
            is_closed: false,
            executable_hash: None,
        },
    )
    .await
    .unwrap();

    rpc_mock
        .expect_get_multiple_accounts_for(
            &[program_id],
            vec![Some(account_value(
                &bpf_loader_upgradeable::ID,
                &program_account_bytes(&pda),
                1_000_000,
            ))],
        )
        .await;
    rpc_mock
        .expect_get_multiple_accounts_for(
            &[pda],
            vec![Some(account_value(
                &bpf_loader_upgradeable::ID,
                // None authority -> frozen
                &program_data_account_bytes(0, None, b"some bytecode"),
                1_000_000,
            ))],
        )
        .await;

    sweep::run_once(&db, &rpc).await.expect("sweep");

    let state = db.get_program_state(&addr).await.unwrap().unwrap();
    assert!(state.is_frozen, "no authority -> frozen");
}

#[tokio::test]
async fn sweep_bootstraps_state_for_orphan_build() {
    let (db, rpc, rpc_mock, _pg) = boot_for_sweep().await;
    let program_id = Pubkey::new_unique();
    let pda = program_data_pda(&program_id);
    let authority = Pubkey::new_unique();
    let bytecode = b"orphan program bytes";
    let expected_hash = compute_program_hash(bytecode);

    // Build row but no program_state row -- common after a missed webhook.
    let addr = Address(program_id);
    let build_id = db
        .insert_build(&NewBuild {
            repository: "https://github.com/x/y".to_string(),
            commit_hash: Some("deadbeef".to_string()),
            program_id: addr,
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
        .unwrap();
    db.mark_build_completed(build_id, &addr, &expected_hash)
        .await
        .unwrap();

    rpc_mock
        .expect_get_multiple_accounts_for(
            &[program_id],
            vec![Some(account_value(
                &bpf_loader_upgradeable::ID,
                &program_account_bytes(&pda),
                1_000_000,
            ))],
        )
        .await;
    rpc_mock
        .expect_get_multiple_accounts_for(
            &[pda],
            vec![Some(account_value(
                &bpf_loader_upgradeable::ID,
                &program_data_account_bytes(0, Some(&authority), bytecode),
                1_000_000,
            ))],
        )
        .await;

    assert!(
        db.get_program_state(&addr).await.unwrap().is_none(),
        "no state row pre-sweep"
    );

    sweep::run_once(&db, &rpc).await.expect("sweep");

    let state = db.get_program_state(&addr).await.unwrap().unwrap();
    assert_eq!(state.on_chain_hash.as_deref(), Some(expected_hash.as_str()));
}

#[tokio::test]
async fn sweep_batches_at_chunk_size() {
    let (db, rpc, rpc_mock, _pg) = boot_for_sweep().await;

    // Seed 105 program_state rows so the sweep has to split into two
    // getMultipleAccounts calls (chunk size = 100).
    for _ in 0..105 {
        db.upsert_program_state(
            &Address(Pubkey::new_unique()),
            &ProgramOnchainState {
                authority: None,
                is_frozen: false,
                is_closed: false,
                executable_hash: None,
            },
        )
        .await
        .unwrap();
    }

    // Return None for every account regardless of which chunk -- we
    // only care about how many RPC calls were made.
    rpc_mock.expect_get_multiple_accounts(vec![None; 100]).await;

    sweep::run_once(&db, &rpc).await.expect("sweep");

    let calls = rpc_mock.method_call_count("getMultipleAccounts").await;
    assert_eq!(
        calls, 2,
        "expected exactly 2 getMultipleAccounts calls (100 + 5), got {calls}"
    );
}
