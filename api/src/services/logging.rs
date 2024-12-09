use crate::Result;
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};
use tracing::{error, info};

/// The directory where logs are stored
const LOGS_DIR: &str = "/logs";

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
pub fn write_logs(std_err: &str, std_out: &str, file_name: &str) -> Result<()> {
    let logs_dir = Path::new(LOGS_DIR);

    // Ensure logs directory exists
    if !logs_dir.exists() {
        error!("Logs directory does not exist: {}", LOGS_DIR);
        return Err(
            std::io::Error::new(std::io::ErrorKind::NotFound, "Logs directory not found").into(),
        );
    }

    // Write stderr log
    let err_path = get_log_path(file_name, "err");
    write_log_file(&err_path, std_err).map_err(|e| {
        error!("Failed to write stderr log: {}", e);
        e
    })?;

    // Write stdout log
    let out_path = get_log_path(file_name, "out");
    write_log_file(&out_path, std_out).map_err(|e| {
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
pub fn read_logs(file_name: &str) -> Value {
    let err_path = get_log_path(file_name, "err");
    let out_path = get_log_path(file_name, "out");

    // Read log contents
    let std_err = std::fs::read_to_string(&err_path).unwrap_or_else(|e| {
        error!("Failed to read stderr log: {}", e);
        String::new()
    });

    let std_out = std::fs::read_to_string(&out_path).unwrap_or_else(|e| {
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

/// Constructs the full path for a log file
fn get_log_path(file_name: &str, log_type: &str) -> PathBuf {
    Path::new(LOGS_DIR).join(format!("{}_{}.log", file_name, log_type))
}

/// Writes content to a log file
fn write_log_file(path: &Path, content: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    file.write_all(content.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_log_file_operations() {
        // Create temporary directory for test logs
        let temp_dir = tempdir().unwrap();
        let test_logs_dir = temp_dir.path().join("logs");
        fs::create_dir(&test_logs_dir).unwrap();

        // Test data
        let file_name = "test_build";
        let std_out = "Build successful";
        let std_err = "Warning: deprecated feature";

        // Write logs
        let err_path = test_logs_dir.join(format!("{}_err.log", file_name));
        let out_path = test_logs_dir.join(format!("{}_out.log", file_name));

        write_log_file(&err_path, std_err).unwrap();
        write_log_file(&out_path, std_out).unwrap();

        // Verify file contents
        assert_eq!(fs::read_to_string(&err_path).unwrap(), std_err);
        assert_eq!(fs::read_to_string(&out_path).unwrap(), std_out);
    }
}
