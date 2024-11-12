use crate::Result;
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;

pub fn write_logs(std_err: &str, std_out: &str, file_name: &str) -> Result<()> {
    // Create a file with name as filename_err.log and write the std_err log to the file
    let mut err_file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(format!("/logs/{}_err.log", file_name))?;
    err_file.write_all(std_err.as_bytes())?;

    // Create a file with name as filename_out.log and write the std_out log to the file
    let mut out_file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(format!("/logs/{}_out.log", file_name))?;
    out_file.write_all(std_out.as_bytes())?;

    Ok(())
}

pub fn read_logs(file_name: &str) -> Value {
    // Read the contents of the file with name as filename_err.log
    let std_err =
        std::fs::read_to_string(format!("/logs/{}_err.log", file_name)).unwrap_or_default();

    // Read the contents of the file with name as filename_out.log
    let std_out =
        std::fs::read_to_string(format!("/logs/{}_out.log", file_name)).unwrap_or_default();

    if std_err.is_empty() && std_out.is_empty() {
        return json!({
            "error": "We could not find the logs for this program"
        });
    }

    // Return the logs as a JSON object
    json!({
        "std_err": std_err,
        "std_out": std_out,
    })
}
