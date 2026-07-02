//! Shared test harness: per-test postgres, axum router builder, and a
//! few HTTP helpers wired around `tower::oneshot`. Wiremock-based RPC
//! mocking lives in [`rpc`].
//!
//! Each test file in `tests/` declares `mod common;` to pull this in;
//! `clippy::dead_code` is muted because not every file uses every helper.

#![allow(dead_code)]

pub mod rpc;

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

pub const AUTH_SECRET: &str = "integration-test-secret";
pub const RPC_URL: &str = "http://127.0.0.1:1";

/// Spins up a fresh postgres, runs migrations, and returns an axum
/// `Router` plus the container handle (held so the container outlives
/// the test).
pub async fn boot() -> (axum::Router, Option<ContainerAsync<Postgres>>) {
    let (router, _db, container) = boot_with_rpc(RPC_URL).await;
    (router, container)
}

/// Same as [`boot`] but exposes the `DbClient` for direct setup /
/// assertions and accepts a custom RPC URL (typically a `wiremock`
/// server's URI).
pub async fn boot_with_rpc(
    rpc_url: &str,
) -> (axum::Router, DbClient, Option<ContainerAsync<Postgres>>) {
    let (url, container) = pg_for_test().await;
    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("db connect");
    db.migrate().await.expect("migrate");
    let state = AppState::new(db.clone(), rpc_url, AUTH_SECRET, 300, 5);
    (initialize_router(state), db, container)
}

/// Returns a connection URL for a fresh, empty postgres database. Uses
/// `TEST_DATABASE_URL` (admin role assumed) with a per-test `CREATE
/// DATABASE` if set, otherwise spins up a one-shot container.
pub async fn pg_for_test() -> (String, Option<ContainerAsync<Postgres>>) {
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

/// `tower_governor::SmartIpKeyExtractor` reads `ConnectInfo` or
/// `x-real-ip`; `oneshot` doesn't populate `ConnectInfo`, so we inject
/// `x-real-ip` on every request.
pub async fn send(app: axum::Router, mut req: Request<Body>) -> (StatusCode, Value) {
    req.headers_mut()
        .insert("x-real-ip", "127.0.0.1".parse().unwrap());
    let resp = app.oneshot(req).await.expect("oneshot");
    let status = resp.status();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

pub async fn get(app: axum::Router, path: &str) -> (StatusCode, Value) {
    send(app, Request::get(path).body(Body::empty()).unwrap()).await
}

pub async fn post(app: axum::Router, path: &str, body: &str) -> (StatusCode, Value) {
    send(
        app,
        Request::post(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap(),
    )
    .await
}

pub async fn post_with_auth(
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

/// Polls `check` every 20 ms until it returns true or `timeout` elapses.
/// Use for the `/pda` and `/unverify` handlers, which `tokio::spawn`
/// their work and return 200 before the DB is updated.
pub async fn wait_until<F, Fut>(timeout: std::time::Duration, mut check: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if check().await {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    false
}
