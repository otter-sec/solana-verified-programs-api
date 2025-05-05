use anyhow::Context;
use std::fs;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, Registry};

pub fn setup_logging() -> Result<(), anyhow::Error> {
    // Create a directory for logs if it doesn't exist
    fs::create_dir_all("./logs").context("Failed to create logs directory")?;

    // Configure a rolling file appender (one file per day)
    let file_appender = rolling::daily("/logs", "app.log");

    // File logging layer (no ANSI)
    let file_layer = fmt::layer().with_writer(file_appender).with_ansi(false); // Optional: adjust level

    // Stdout logging layer (with ANSI)
    let stdout_layer = fmt::layer().with_writer(std::io::stdout);

    // Combine both layers into a subscriber
    let subscriber = Registry::default().with(stdout_layer).with(file_layer);

    // Set global default
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set global default subscriber")?;

    Ok(())
}
