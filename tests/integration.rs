//! Integration tests covering the routing layer + DB queries against a
//! real postgres (via testcontainers). RPC-dependent paths (verify*) are
//! skipped — they'd need a Solana RPC mock.
//!
//! No env var setup, no globals — `AppState` is constructed per test
//! with a placeholder RPC URL and a test auth secret. Each test gets
//! its own postgres database (a fresh container, or `CREATE DATABASE`
//! against `TEST_DATABASE_URL` if set).

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::{runners::AsyncRunner, ContainerAsync};
use tower::ServiceExt;
use verified_programs_api::api::routes::initialize_router;
use verified_programs_api::db::DbClient;
use verified_programs_api::state::AppState;

const AUTH_SECRET: &str = "integration-test-secret";
const RPC_URL: &str = "http://127.0.0.1:1";

/// Spins up a fresh postgres, runs the v2 migration against it, and
/// returns an axum `Router` plus the container handle (kept so the
/// container outlives the test).
async fn boot() -> (axum::Router, Option<ContainerAsync<Postgres>>) {
    let (router, _db, container) = boot_with_rpc(RPC_URL).await;
    (router, container)
}

/// Same as `boot` but exposes the `DbClient` for direct setup/assertions
/// and accepts a custom RPC URL (typically a `wiremock` server's URI).
async fn boot_with_rpc(
    rpc_url: &str,
) -> (axum::Router, DbClient, Option<ContainerAsync<Postgres>>) {
    let (url, container) = pg_for_test().await;
    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("db connect");
    db.migrate().await.expect("migrate");
    let state = AppState::new(db.clone(), rpc_url, AUTH_SECRET, 300);
    (initialize_router(state), db, container)
}

async fn pg_for_test() -> (String, Option<ContainerAsync<Postgres>>) {
    // `TEST_DATABASE_URL` should point at an admin role; we create a
    // fresh database per test so they don't fight over rows.
    if let Ok(admin_url) = std::env::var("TEST_DATABASE_URL") {
        let db_name = format!("test_{}", uuid::Uuid::new_v4().simple());
        let admin = sqlx::PgPool::connect(&admin_url).await.expect("admin");
        let sql = format!("CREATE DATABASE \"{db_name}\"");
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&admin)
            .await
            .expect("create db");
        drop(admin);
        let base = admin_url.rsplit_once('/').expect("url has db").0;
        return (format!("{base}/{db_name}"), None);
    }
    let pg = Postgres::default().start().await.expect("start postgres");
    let port = pg
        .get_host_port_ipv4(5432)
        .await
        .expect("postgres host port");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    (url, Some(pg))
}

/// `tower_governor::SmartIpKeyExtractor` reads ConnectInfo or `x-real-ip`;
/// `oneshot` doesn't set up `ConnectInfo`, so we inject `x-real-ip` on
/// every request.
async fn send(app: axum::Router, mut req: Request<Body>) -> (StatusCode, Value) {
    req.headers_mut()
        .insert("x-real-ip", "127.0.0.1".parse().unwrap());
    let resp = app.oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn get(app: axum::Router, path: &str) -> (StatusCode, Value) {
    send(app, Request::get(path).body(Body::empty()).unwrap()).await
}

async fn post(app: axum::Router, path: &str, body: &str) -> (StatusCode, Value) {
    send(
        app,
        Request::post(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap(),
    )
    .await
}

async fn post_with_auth(
    app: axum::Router,
    path: &str,
    auth: &str,
    body: &str,
) -> (StatusCode, Value) {
    send(
        app,
        Request::post(path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, auth)
            .body(Body::from(body.to_owned()))
            .unwrap(),
    )
    .await
}

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

/// Seeds a program with two completed builds at the same on-chain hash,
/// one from a whitelisted signer (`trusted_repo`) and one from a random
/// signer (different repo), with the untrusted row strictly newer on
/// `completed_at` so it would win the `ORDER BY completed_at DESC`
/// tiebreaker absent the trust filter. `program_state.authority` is
/// left NULL so the per-program-authority branch can't accidentally
/// match the untrusted row.
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

/// End-to-end smoke test: hits `/verify_sync` for a currently-verified
/// program on mainnet and asserts the response reports `is_verified:
/// true`. Drives the full pipeline: PDA lookup, `solana-verify` build
/// in Docker, hash compare.
///
/// Slow (typically 5-15 min) and depends on Docker Hub, GitHub, and
/// mainnet RPC. Ignored by default; CI runs it on a weekly cron via
/// `.github/workflows/verify-smoke.yaml`.
///
/// Run locally:
///   cargo test --test integration verify_smoke -- --ignored --nocapture
#[tokio::test]
#[ignore = "slow; spawns solana-verify build in Docker against mainnet"]
async fn verify_smoke_otter_verify_program() {
    let rpc_url = std::env::var("VERIFY_SMOKE_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let (app, _db, _pg) = boot_with_rpc(&rpc_url).await;

    // Otter Verify itself: small, currently verified, repo owned by otter-sec.
    const PROGRAM_ID: &str = "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC";
    let body = format!(r#"{{"program_id":"{PROGRAM_ID}"}}"#);

    let (status, response) = post(app, "/verify_sync", &body).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "/verify_sync returned {status}: {response:#?}"
    );
    assert_eq!(
        response["is_verified"], true,
        "verify smoke failed - program did not verify: {response:#?}"
    );
}

/// Boots a fresh postgres with the v1 (Diesel-era) schema pre-seeded,
/// then runs the v2 migrations on top and confirms the data ended up
/// in the new tables with the right shape.
#[tokio::test]
async fn migrates_v1_schema_to_v2() {
    let (url, _pg) = pg_for_test().await;
    let pool = sqlx::PgPool::connect(&url).await.expect("pool");

    sqlx::raw_sql(
        r#"
        CREATE TABLE solana_program_builds (
            id VARCHAR(36) PRIMARY KEY,
            repository VARCHAR NOT NULL,
            commit_hash VARCHAR,
            program_id VARCHAR(44) NOT NULL,
            lib_name VARCHAR,
            base_docker_image VARCHAR,
            mount_path VARCHAR,
            cargo_args TEXT[],
            bpf_flag BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMP NOT NULL DEFAULT NOW(),
            status VARCHAR(20) NOT NULL,
            signer VARCHAR,
            arch VARCHAR(3)
        );
        CREATE TABLE verified_programs (
            id VARCHAR(36) PRIMARY KEY,
            program_id VARCHAR(44) NOT NULL,
            is_verified BOOLEAN NOT NULL,
            on_chain_hash VARCHAR NOT NULL,
            executable_hash VARCHAR NOT NULL,
            verified_at TIMESTAMP NOT NULL DEFAULT NOW(),
            solana_build_id VARCHAR(36) NOT NULL REFERENCES solana_program_builds(id)
        );
        CREATE TABLE program_authority (
            program_id VARCHAR(44) PRIMARY KEY,
            authority_id VARCHAR(44),
            last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
            is_frozen BOOLEAN DEFAULT FALSE,
            is_closed BOOLEAN NOT NULL DEFAULT FALSE
        );
        CREATE TABLE build_logs (
            id VARCHAR(36) PRIMARY KEY,
            program_address VARCHAR(44) NOT NULL,
            file_name VARCHAR NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT NOW()
        );

        INSERT INTO solana_program_builds VALUES
          ('11111111-1111-1111-1111-111111111111', 'https://github.com/a/b', NULL,
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', NULL, NULL, NULL, NULL,
           true, NOW(), 'completed', '9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU', NULL);
        INSERT INTO verified_programs VALUES
          ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC',
           true, 'hash_v', 'hash_v', NOW(),
           '11111111-1111-1111-1111-111111111111');
        INSERT INTO program_authority VALUES
          ('verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC',
           '9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU',
           NOW(), false, false);
        INSERT INTO build_logs VALUES
          ('44444444-4444-4444-4444-444444444444',
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', 'log_abc', NOW());
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed v1");

    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("db connect");
    db.migrate().await.expect("migrate v1 -> v2");

    let builds: Vec<(uuid::Uuid, String, Option<String>)> =
        sqlx::query_as("SELECT id, program_id, executable_hash FROM builds ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("builds");
    assert_eq!(builds.len(), 1);
    assert_eq!(builds[0].1, "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC");
    assert_eq!(builds[0].2.as_deref(), Some("hash_v"));

    type StateRow = (String, Option<String>, Option<String>, bool, bool);
    let state: Vec<StateRow> = sqlx::query_as(
        "SELECT program_id, on_chain_hash, authority, is_frozen, is_closed
         FROM program_state",
    )
    .fetch_all(&pool)
    .await
    .expect("state");
    assert_eq!(state.len(), 1);
    assert_eq!(state[0].1.as_deref(), Some("hash_v"));
    assert_eq!(
        state[0].2.as_deref(),
        Some("9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU")
    );

    let logs_row: (String, String) = sqlx::query_as("SELECT program_id, file_name FROM build_logs")
        .fetch_one(&pool)
        .await
        .expect("build_logs");
    assert_eq!(logs_row.1, "log_abc");

    let v1_exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS (
             SELECT 1 FROM information_schema.tables
             WHERE table_name = 'solana_program_builds'
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("v1 check");
    assert!(!v1_exists.0, "v1 tables should be dropped");
}
