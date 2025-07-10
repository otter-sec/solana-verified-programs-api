use crate::db::DbClient;
use axum::http::Request;
use axum::{
    error_handling::HandleErrorLayer,
    http::{Method, StatusCode},
    response::Response,
    routing::{get, post},
    BoxError, Router,
};
use std::time::Duration;
use tower::{buffer::BufferLayer, limit::RateLimitLayer, ServiceBuilder};
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::Span;

use super::{handlers::*, index::index};

pub fn initialize_router(db: DbClient) -> Router {
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
        .make_span_with(|request: &Request<_>| {
            let uri = request.uri();
            let method = request.method();
            tracing::info_span!(
                "http_request",
                method = %method,
                uri = %uri,
                path = uri.path(),
            )
        })
        .on_request(|request: &Request<_>, _span: &Span| {
            tracing::info!(
                method = %request.method(),
                path = request.uri().path(),
                "started processing request"
            );
        })
        .on_response(
            |response: &Response, latency: std::time::Duration, _span: &Span| {
                tracing::info!(
                    latency = ?latency,
                    status = response.status().as_u16(),
                    "finished processing request"
                );
            },
        );

    // Define routes with their rate limits
    Router::new()
        // Verification routes (stricter rate limits)
        .route("/verify", post(process_async_verification))
        .route(
            "/verify-with-signer",
            post(process_async_verification_with_signer),
        )
        .route("/verify_sync", post(process_sync_verification))
        .layer(
            global_rate_limit(5)
                .layer(rate_limit_per_ip(30, 1))
                .layer(cors(Method::POST))
                .layer(CompressionLayer::new().zstd(true)),
        )
        .route("/unverify", post(handle_unverify))
        .layer(
            global_rate_limit(100)
                .layer(rate_limit_per_ip(1, 100))
                .layer(cors(Method::POST))
                .layer(CompressionLayer::new().zstd(true)),
        )
        .route("/status-all/:address", get(get_verification_status_all))
        .route("/status/:address", get(get_verification_status))
        .route("/job/:job_id", get(get_job_status))
        .route("/logs/:address", get(get_build_logs))
        .route("/pda", post(handle_pda_updates_creations))
        .route("/verified-programs", get(get_verified_programs_list))
        .route(
            "/verified-programs/:page",
            get(get_verified_programs_list_paginated),
        )
        .route(
            "/verified-programs-status",
            get(get_verified_programs_status),
        )
        .layer(
            global_rate_limit(10000)
                .layer(rate_limit_per_ip(1, 100))
                .layer(cors(Method::GET))
                .layer(CompressionLayer::new().zstd(true)),
        )
        // Base route
        .route("/", get(|| async { index() }))
        .route("/health", get(|| async { StatusCode::OK }))
        // Apply common middleware
        .layer(trace_layer)
        .with_state(db)
}
