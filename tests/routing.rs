//! Routing-layer tests: per-endpoint shape + validation + DB query
//! results against a real postgres. RPC-dependent paths (verify*) are
//! covered by the smoke test in `verify_smoke.rs`.

mod common;

use axum::http::StatusCode;
use common::{boot, boot_with_rpc, get, RPC_URL};
use verified_programs_api::db::DbClient;

#[tokio::test]
async fn health_background_jobs_returns_accepted_for_unknown() {
    let (app, _pg) = boot().await;
    let (status, body) = get(app, "/health/background-jobs").await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(body["status"], "unknown");
}

#[tokio::test]
async fn verified_programs_empty_db() {
    let (app, _pg) = boot().await;
    let (status, body) = get(app, "/verified-programs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["meta"]["total"], 0);
    assert!(body["verified_programs"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn verified_programs_status_empty_db() {
    let (app, _pg) = boot().await;
    let (status, body) = get(app, "/verified-programs-status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "success");
    assert!(body["data"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn status_for_unknown_program_is_not_verified() {
    let (app, _pg) = boot().await;
    let (status, body) = get(app, "/status/verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["is_verified"], false);
    assert_eq!(body["is_closed"], false);
    assert_eq!(body["is_frozen"], false);
}

#[tokio::test]
async fn status_for_invalid_address_is_400() {
    let (app, _pg) = boot().await;
    let (status, _) = get(app, "/status/not-a-real-pubkey").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn status_all_for_unknown_program_empty() {
    let (app, _pg) = boot().await;
    let (status, body) = get(
        app,
        "/status-all/verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn resolve_hash_empty_db() {
    let (app, _pg) = boot().await;
    let (status, body) = get(app, "/resolve-hash/deadbeef").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["executable_hash"], "deadbeef");
    assert!(body["builds"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn job_status_invalid_uuid_returns_unknown() {
    let (app, _pg) = boot().await;
    let (status, body) = get(app, "/job/not-a-uuid").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "unknown");
}

#[tokio::test]
async fn job_status_unknown_uuid_returns_not_found_message() {
    let (app, _pg) = boot().await;
    let (status, body) = get(app, "/job/00000000-0000-0000-0000-000000000000").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "unknown");
    assert_eq!(body["message"], "Job not found");
}

#[tokio::test]
async fn logs_missing_returns_404() {
    let (app, _pg) = boot().await;
    let (status, _) = get(app, "/logs/00000000-0000-0000-0000-000000000000").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn logs_invalid_uuid_returns_400() {
    let (app, _pg) = boot().await;
    let (status, _) = get(app, "/logs/garbage").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// One of the whitelisted `SIGNER_KEYS` from `onchain::otter`.
const TRUSTED_SIGNER: &str = "9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU";

/// Trust-filter regression test for `GET /status`.
///
/// Seeds two completed builds for the same program with the same
/// executable hash but different repo URLs: one from a whitelisted
/// `SIGNER_KEYS` signer, one from a random (untrusted) signer. The
/// untrusted build is marked completed last, so without a trust filter
/// the LATERAL join's `ORDER BY completed_at DESC` surfaces its
/// `repo_url`. With the filter, the trusted row wins.
#[tokio::test]
#[ignore = "fails until trust filter ships on /status; tracked separately"]
async fn status_filters_untrusted_signer() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    let seed = seed_trusted_vs_untrusted(&db).await;

    let (status, body) = get(app, &format!("/status/{}", seed.program_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["is_verified"], true);
    assert_eq!(
        body["repo_url"], seed.trusted_repo,
        "untrusted signer's repo_url leaked through /status"
    );
}

/// Trust-filter regression test for `GET /verified-programs-status`.
///
/// Same seed as `status_filters_untrusted_signer`; asserts the bulk
/// endpoint's row for the seeded program surfaces the trusted signer's
/// metadata, not the untrusted one's.
#[tokio::test]
#[ignore = "fails until trust filter ships on /verified-programs-status; tracked separately"]
async fn verified_programs_status_filters_untrusted_signer() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    let seed = seed_trusted_vs_untrusted(&db).await;

    let (status, body) = get(app, "/verified-programs-status").await;
    assert_eq!(status, StatusCode::OK);
    let entries = body["data"].as_array().expect("data array");
    let entry = entries
        .iter()
        .find(|e| e["program_id"] == seed.program_id)
        .expect("entry for seeded program");
    assert_eq!(
        entry["repo_url"], seed.trusted_repo,
        "untrusted signer's repo_url leaked through /verified-programs-status"
    );
}

struct TrustSetup {
    program_id: String,
    trusted_repo: String,
}

/// Seeds a program with two completed builds at the same on-chain hash:
/// one from a whitelisted signer (`trusted_repo`) and one from a random
/// signer (different repo), with the untrusted row strictly newer on
/// `completed_at` so it would win the `ORDER BY completed_at DESC`
/// tiebreaker absent the trust filter. `program_state.authority` is
/// NULL so the per-program-authority branch can't accidentally match
/// the untrusted row.
async fn seed_trusted_vs_untrusted(db: &DbClient) -> TrustSetup {
    use std::str::FromStr;
    use verified_programs_api::db::NewBuild;
    use verified_programs_api::onchain::ProgramOnchainState;
    use verified_programs_api::types::Address;

    const TEST_HASH: &str = "0000000000000000000000000000000000000000000000000000000000001234";
    const TRUSTED_REPO: &str = "https://github.com/trusted/repo";
    const UNTRUSTED_REPO: &str = "https://github.com/untrusted/repo";

    let program_id = Address(solana_pubkey::Pubkey::new_unique());
    let trusted = Address::from_str(TRUSTED_SIGNER).unwrap();
    let untrusted = Address(solana_pubkey::Pubkey::new_unique());

    db.upsert_program_state(
        &program_id,
        &ProgramOnchainState {
            authority: None,
            is_frozen: false,
            is_closed: false,
            executable_hash: Some(TEST_HASH.to_string()),
        },
    )
    .await
    .expect("upsert state");

    let make_build = |signer: Address, repo: &str| NewBuild {
        repository: repo.to_string(),
        commit_hash: None,
        program_id,
        lib_name: None,
        base_docker_image: None,
        mount_path: None,
        cargo_args: None,
        cargo_build_sbf_args: None,
        bpf_flag: false,
        arch: None,
        signer: Some(signer),
    };

    let trusted_id = db
        .insert_build(&make_build(trusted, TRUSTED_REPO))
        .await
        .expect("insert trusted");
    db.mark_build_completed(trusted_id, &program_id, TEST_HASH)
        .await
        .expect("complete trusted");

    // postgres `NOW()` has microsecond resolution but two consecutive
    // calls can tie. Sleep so the untrusted row is strictly newer on
    // `completed_at`.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let untrusted_id = db
        .insert_build(&make_build(untrusted, UNTRUSTED_REPO))
        .await
        .expect("insert untrusted");
    db.mark_build_completed(untrusted_id, &program_id, TEST_HASH)
        .await
        .expect("complete untrusted");

    TrustSetup {
        program_id: program_id.to_string(),
        trusted_repo: TRUSTED_REPO.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Read paths with seeded data.
// ---------------------------------------------------------------------------

/// A 64-hex hash that's stable across tests; the value doesn't matter,
/// only that it's deterministic.
const SEED_HASH: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
const SEED_REPO: &str = "https://github.com/seed/program";
const SEED_COMMIT: &str = "f00dcafe";

struct SeedOpts {
    on_chain_hash: Option<String>,
    executable_hash: Option<String>,
    is_frozen: bool,
    is_closed: bool,
}

impl Default for SeedOpts {
    fn default() -> Self {
        Self {
            on_chain_hash: Some(SEED_HASH.to_string()),
            executable_hash: Some(SEED_HASH.to_string()),
            is_frozen: false,
            is_closed: false,
        }
    }
}

/// Seeds one verified program: `program_state` with the given on-chain
/// hash + flags, plus one completed build under a whitelisted signer.
/// Returns the program id.
async fn seed_program(db: &DbClient, opts: SeedOpts) -> verified_programs_api::types::Address {
    use std::str::FromStr;
    use verified_programs_api::db::NewBuild;
    use verified_programs_api::onchain::ProgramOnchainState;
    use verified_programs_api::types::Address;

    let program_id = Address(solana_pubkey::Pubkey::new_unique());
    let trusted = Address::from_str(TRUSTED_SIGNER).unwrap();

    db.upsert_program_state(
        &program_id,
        &ProgramOnchainState {
            authority: None,
            is_frozen: opts.is_frozen,
            is_closed: opts.is_closed,
            executable_hash: opts.on_chain_hash,
        },
    )
    .await
    .expect("upsert state");

    let id = db
        .insert_build(&NewBuild {
            repository: SEED_REPO.to_string(),
            commit_hash: Some(SEED_COMMIT.to_string()),
            program_id,
            lib_name: None,
            base_docker_image: None,
            mount_path: None,
            cargo_args: None,
            cargo_build_sbf_args: None,
            bpf_flag: false,
            arch: None,
            signer: Some(trusted),
        })
        .await
        .expect("insert build");

    if let Some(eh) = opts.executable_hash {
        db.mark_build_completed(id, &program_id, &eh)
            .await
            .expect("complete build");
    }

    program_id
}

#[tokio::test]
async fn status_for_verified_program() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    let program_id = seed_program(&db, SeedOpts::default()).await;

    let (status, body) = get(app, &format!("/status/{program_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["is_verified"], true);
    assert_eq!(body["on_chain_hash"], SEED_HASH);
    assert_eq!(body["executable_hash"], SEED_HASH);
    assert_eq!(body["repo_url"], format!("{SEED_REPO}/tree/{SEED_COMMIT}"));
    assert_eq!(body["commit"], SEED_COMMIT);
    assert_eq!(body["is_frozen"], false);
    assert_eq!(body["is_closed"], false);
}

#[tokio::test]
async fn status_for_mismatched_hash() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    let program_id = seed_program(
        &db,
        SeedOpts {
            on_chain_hash: Some(SEED_HASH.to_string()),
            executable_hash: Some(
                "1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            ),
            ..SeedOpts::default()
        },
    )
    .await;

    let (status, body) = get(app, &format!("/status/{program_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["is_verified"], false);
    assert_eq!(body["on_chain_hash"], SEED_HASH);
}

#[tokio::test]
async fn status_for_closed_program() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    let program_id = seed_program(
        &db,
        SeedOpts {
            is_closed: true,
            ..SeedOpts::default()
        },
    )
    .await;

    let (status, body) = get(app, &format!("/status/{program_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["is_verified"], false,
        "closed programs are never verified regardless of hash"
    );
    assert_eq!(body["is_closed"], true);
}

#[tokio::test]
async fn status_for_frozen_program() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    let program_id = seed_program(
        &db,
        SeedOpts {
            is_frozen: true,
            ..SeedOpts::default()
        },
    )
    .await;

    let (status, body) = get(app, &format!("/status/{program_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["is_frozen"], true);
    assert_eq!(
        body["is_verified"], true,
        "frozen programs can still be verified"
    );
}

#[tokio::test]
async fn verified_programs_lists_seeded_programs() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    let a = seed_program(&db, SeedOpts::default()).await;
    let b = seed_program(&db, SeedOpts::default()).await;

    let (status, body) = get(app, "/verified-programs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["meta"]["total"], 2);
    let listed: Vec<&str> = body["verified_programs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(listed.iter().any(|p| *p == a.to_string()));
    assert!(listed.iter().any(|p| *p == b.to_string()));
}

#[tokio::test]
async fn verified_programs_paginated() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    for _ in 0..30 {
        seed_program(&db, SeedOpts::default()).await;
    }

    let (status, body) = get(app, "/verified-programs/2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["meta"]["total"], 30);
    assert_eq!(body["meta"]["page"], 2);
    assert_eq!(body["meta"]["total_pages"], 2);
    assert_eq!(body["meta"]["has_prev_page"], true);
    assert_eq!(body["meta"]["has_next_page"], false);
    // PER_PAGE = 20, so page 2 has the remaining 10.
    assert_eq!(body["verified_programs"].as_array().unwrap().len(), 10);
}

#[tokio::test]
async fn verified_programs_status_returns_seeded() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    let a = seed_program(&db, SeedOpts::default()).await;
    let b = seed_program(&db, SeedOpts::default()).await;

    let (status, body) = get(app, "/verified-programs-status").await;
    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().expect("data array");
    assert_eq!(data.len(), 2);
    let ids: Vec<&str> = data
        .iter()
        .map(|e| e["program_id"].as_str().unwrap())
        .collect();
    assert!(ids.iter().any(|p| *p == a.to_string()));
    assert!(ids.iter().any(|p| *p == b.to_string()));
    for entry in data {
        assert_eq!(entry["is_verified"], true);
        assert_eq!(entry["on_chain_hash"], SEED_HASH);
    }
}

#[tokio::test]
async fn resolve_hash_returns_matching_with_flag() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    // Two builds at the same executable hash, but only program `a`'s
    // on-chain hash matches it (b's on-chain hash differs).
    let a = seed_program(&db, SeedOpts::default()).await;
    let _b = seed_program(
        &db,
        SeedOpts {
            on_chain_hash: Some(
                "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            ),
            ..SeedOpts::default()
        },
    )
    .await;

    let (status, body) = get(app, &format!("/resolve-hash/{SEED_HASH}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["executable_hash"], SEED_HASH);
    let builds = body["builds"].as_array().expect("builds array");
    assert_eq!(builds.len(), 2);

    let entry_a = builds
        .iter()
        .find(|e| e["program_id"] == a.to_string())
        .expect("entry for a");
    assert_eq!(entry_a["matches_deployed"], true);

    let entry_b = builds
        .iter()
        .find(|e| e["program_id"] != a.to_string())
        .expect("entry for b");
    assert_eq!(entry_b["matches_deployed"], false);
}

#[tokio::test]
async fn cache_invalidates_on_write() {
    let (app, db, _pg) = boot_with_rpc(RPC_URL).await;
    let program_id = seed_program(&db, SeedOpts::default()).await;

    // First hit -- populates the moka cache.
    let (_, first) = get(app.clone(), &format!("/status/{program_id}")).await;
    assert_eq!(first["is_verified"], true);

    // Mutate -- `unverify_program` invalidates the cache entry.
    db.unverify_program(
        &program_id,
        "9999999999999999999999999999999999999999999999999999999999999999",
    )
    .await
    .expect("unverify");

    let (_, second) = get(app, &format!("/status/{program_id}")).await;
    assert_eq!(
        second["on_chain_hash"], "9999999999999999999999999999999999999999999999999999999999999999",
        "cache should have been invalidated; got stale response"
    );
    assert_eq!(second["is_verified"], false);
}
