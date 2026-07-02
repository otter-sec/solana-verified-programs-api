//! End-to-end smoke test: drive `/verify_sync` for a real, currently-
//! verified program on mainnet and assert it reports verified.
//!
//! Slow (typically 5-15 min) and depends on Docker Hub, GitHub, and
//! mainnet RPC. Ignored by default; CI runs it on a weekly cron via
//! `.github/workflows/verify-smoke.yaml`.
//!
//! Run locally with:
//!   cargo test --test verify_smoke -- --ignored --nocapture

mod common;

use axum::http::StatusCode;
use common::{boot_with_rpc, post};

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
