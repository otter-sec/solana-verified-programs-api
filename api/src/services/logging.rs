use crate::errors::{ApiError, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{error, info};
use uuid::Uuid;

/// The directory where logs are stored
const LOGS_DIR: &str = "/logs";

/// Solana mainnet RPC URL for replacing sensitive environment values
const SOLANA_MAINNET_RPC: &str = "https://api.mainnet-beta.solana.com";

/// Rejects file names that aren't UUIDs. Defence-in-depth against any
/// caller (or DB row) trying to escape `/logs` via path components.
fn validate_file_name(file_name: &str) -> Result<()> {
    Uuid::parse_str(file_name)
        .map(|_| ())
        .map_err(|_| ApiError::BadRequest(format!("Invalid log file name: {file_name}")))
}

/// Writes build logs to separate files for stdout and stderr
///
/// # Arguments
/// * `std_err` - Standard error output from build process
/// * `std_out` - Standard output from build process
/// * `file_name` - Base name for log files
///
/// # Returns
/// * `Result<()>` - Success or error status
///
/// Creates two files:
/// - `{file_name}_err.log` for stderr
/// - `{file_name}_out.log` for stdout
///
/// Note: Replaces any `rpc_url` occurrences with the Solana mainnet RPC URL.
pub async fn write_logs(
    std_err: &str,
    std_out: &str,
    file_name: &str,
    rpc_url: &str,
) -> Result<()> {
    validate_file_name(file_name)?;
    let logs_dir = Path::new(LOGS_DIR);

    if !logs_dir.exists() {
        error!("Logs directory does not exist: {}", LOGS_DIR);
        return Err(
            std::io::Error::new(std::io::ErrorKind::NotFound, "Logs directory not found").into(),
        );
    }

    let sanitized_stderr = sanitize_log_content(std_err, rpc_url);
    let sanitized_stdout = sanitize_log_content(std_out, rpc_url);

    // Write stderr log
    let err_path = get_log_path(file_name, "err");
    write_log_file(&err_path, &sanitized_stderr)
        .await
        .map_err(|e| {
            error!("Failed to write stderr log: {}", e);
            e
        })?;

    // Write stdout log
    let out_path = get_log_path(file_name, "out");
    write_log_file(&out_path, &sanitized_stdout)
        .await
        .map_err(|e| {
            error!("Failed to write stdout log: {}", e);
            e
        })?;

    info!("Successfully wrote logs for {}", file_name);
    Ok(())
}

/// Reads build logs from files and returns them as JSON
///
/// # Arguments
/// * `file_name` - Base name of log files to read
///
/// # Returns
/// * `Value` - JSON object containing logs or error message
pub async fn read_logs(file_name: &str) -> Value {
    if let Err(e) = validate_file_name(file_name) {
        error!("Refusing to read logs with bad file name: {}", e);
        return json!({ "error": "We could not find the logs for this program" });
    }
    let err_path = get_log_path(file_name, "err");
    let out_path = get_log_path(file_name, "out");

    // Read log contents
    let std_err = fs::read_to_string(&err_path).await.unwrap_or_else(|e| {
        error!("Failed to read stderr log: {}", e);
        String::new()
    });

    let std_out = fs::read_to_string(&out_path).await.unwrap_or_else(|e| {
        error!("Failed to read stdout log: {}", e);
        String::new()
    });

    // Return error if both logs are empty
    if std_err.is_empty() && std_out.is_empty() {
        error!("No logs found for {}", file_name);
        return json!({
            "error": "We could not find the logs for this program"
        });
    }

    json!({
        "std_err": std_err,
        "std_out": std_out,
    })
}

/// Replaces any occurrence of the (potentially secret) `rpc_url` with the
/// Solana mainnet endpoint so build logs don't leak API keys.
fn sanitize_log_content(content: &str, rpc_url: &str) -> String {
    content.replace(rpc_url, SOLANA_MAINNET_RPC)
}

/// Constructs the full path for a log file
fn get_log_path(file_name: &str, log_type: &str) -> PathBuf {
    Path::new(LOGS_DIR).join(format!("{file_name}_{log_type}.log"))
}

/// Writes content to a log file
async fn write_log_file(path: &Path, content: &str) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .await?;

    file.write_all(content.as_bytes()).await?;
    file.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_log_file_operations() {
        let temp_dir = tempdir().unwrap();
        let test_logs_dir = temp_dir.path().join("logs");
        fs::create_dir(&test_logs_dir).unwrap();

        let file_name = "test_build";
        let std_out = "Build successful";
        let std_err = "Warning: deprecated feature";

        let err_path = test_logs_dir.join(format!("{file_name}_err.log"));
        let out_path = test_logs_dir.join(format!("{file_name}_out.log"));

        write_log_file(&err_path, std_err).await.unwrap();
        write_log_file(&out_path, std_out).await.unwrap();

        assert_eq!(std::fs::read_to_string(&err_path).unwrap(), std_err);
        assert_eq!(std::fs::read_to_string(&out_path).unwrap(), std_out);
    }

    #[test]
    fn test_sanitize_log_content() {
        let rpc_url = "https://secret-rpc.example.com/key=abc";
        let test_content = format!("Using RPC URL: {rpc_url}");
        let expected = format!("Using RPC URL: {SOLANA_MAINNET_RPC}");
        assert_eq!(sanitize_log_content(&test_content, rpc_url), expected);

        let unchanged = "Build successful without RPC URL references";
        assert_eq!(sanitize_log_content(unchanged, rpc_url), unchanged);
    }
}
