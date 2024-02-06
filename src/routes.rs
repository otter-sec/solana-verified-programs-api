mod job;
mod status;
mod verified_programs;
mod verify_async;
mod verify_sync;
use crate::db::DbClient;
use crate::routes::{
    job::get_job_status, status::verify_status, verified_programs::get_verified_programs_list,
    verify_async::verify_async, verify_sync::verify_sync,
};
use axum::{
    error_handling::HandleErrorLayer,
    http::{Method, StatusCode},
    routing::{get, post},
    BoxError, Json, Router,
};
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::Duration;
use tower::{buffer::BufferLayer, limit::RateLimitLayer, ServiceBuilder};
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

pub fn create_router(db: DbClient) -> Router {
    let error_handler = || {
        ServiceBuilder::new().layer(HandleErrorLayer::new(|err: BoxError| async move {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Unhandled error: {}", err),
            )
        }))
    };

    let global_rate_limit = |req_per_sec: u64| {
        ServiceBuilder::new()
            .layer(error_handler())
            .layer(BufferLayer::new(1024))
            .layer(RateLimitLayer::new(req_per_sec, Duration::from_secs(1)))
    };

    let rate_limit_per_ip = |timeout: u64, limit: u32| {
        let config = Box::new(
            GovernorConfigBuilder::default()
                .per_second(timeout)
                .burst_size(limit)
                .use_headers()
                .key_extractor(SmartIpKeyExtractor)
                .finish()
                .unwrap(),
        );

        ServiceBuilder::new()
            .layer(error_handler())
            .layer(GovernorLayer {
                config: Box::leak(config),
            })
    };

    let cors = |method: Method| {
        ServiceBuilder::new().layer(CorsLayer::new().allow_methods(method).allow_origin(Any))
    };

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().include_headers(true))
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    Router::new()
        .route("/", get(|| async { index() }))
        .route("/verify", post(verify_async))
        .route("/verify_sync", post(verify_sync))
        .layer(
            global_rate_limit(1)
                .layer(rate_limit_per_ip(30, 1))
                .layer(cors(Method::POST))
                .layer(CompressionLayer::new().zstd(true)),
        )
        .route("/status/:address", get(verify_status))
        .layer(
            global_rate_limit(10000)
                .layer(rate_limit_per_ip(1, 100))
                .layer(cors(Method::GET))
                .layer(CompressionLayer::new().zstd(true)),
        )
        .route("/job/:job_id", get(get_job_status))
        .layer(
            global_rate_limit(10000)
                .layer(rate_limit_per_ip(1, 100))
                .layer(cors(Method::GET))
                .layer(CompressionLayer::new().zstd(true)),
        )
        .route("/verified-programs", get(get_verified_programs_list))
        .layer(
            global_rate_limit(10000)
                .layer(rate_limit_per_ip(1, 100))
                .layer(cors(Method::GET))
                .layer(CompressionLayer::new().zstd(true)),
        )
        .layer(trace_layer)
        .with_state(db)
}

static INDEX_JSON: OnceLock<Value> = OnceLock::new();

fn index() -> Json<Value> {
    let value = INDEX_JSON.get_or_init(||
        json!({
            "endpoints": [
                {
                    "path": "/verify",
                    "method": "POST",
                    "description": "Verify a program",
                    "params" : {
                        "repo": "Git repository URL",
                        "program_id": "Program ID of the program in mainnet",
                        "commit": "(Optional) Commit hash of the repository. If not specified, the latest commit will be used.",
                        "lib_name": "(Optional) If the repository contains multiple programs, specify the name of the library name of the program to build and verify.",
                        "bpf_flag": "(Optional)  If the program requires cargo build-bpf (instead of cargo build-sbf), as for an Anchor program, set this flag.",
                        "base_image": "(Optional) Base docker image to use for building the program.",
                        "mount_path": "(Optional) Mount path for the repository.",
                        "cargo_args": "(Optional) Cargo args to pass to the build command. It should be Vector of strings."
                    }
                },
            ]
        })
    );
    Json(value.clone())
}
