use anyhow::Context;
use chrono::Utc;
use serde_json::Value;
use std::fs;
use tracing::info;
use tracing_appender::rolling;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::{fmt, layer::SubscriberExt, Layer, Registry};

pub fn setup_logging() -> Result<(), anyhow::Error> {
    // Ensure /logs exists
    fs::create_dir_all("/logs").context("Failed to create logs directory")?;

    // Daily rotating file appender
    let file_appender = rolling::daily("/logs", "app.log");

    // Targets that should log everything (TRACE includes DEBUG, INFO, WARN, ERROR)
    let target_filter = Targets::new().with_target("save_to_log_file", LevelFilter::TRACE);

    // File layer with filtered targets
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_filter(target_filter);

    // Stdout layer logs everything
    let stdout_layer = fmt::layer().with_writer(std::io::stdout);

    // Combine both layers
    let subscriber = Registry::default().with(stdout_layer).with(file_layer);

    // Set as global subscriber
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set global default subscriber")?;

    Ok(())
}

pub fn log_to_file(method: &str, path: &str, body: Option<&Value>) {
    let timestamp = Utc::now().to_rfc3339();

    match body {
        Some(b) => {
            info!(
                target: "save_to_log_file",
                method = method,
                uri = path,
                body = %b,
                "{} {} {} {}", timestamp, method, path, b
            );
        }
        None => {
            info!(
                target: "save_to_log_file",
                method = method,
                uri = path,
                "{} {} {}", timestamp, method, path
            );
        }
    }
}
