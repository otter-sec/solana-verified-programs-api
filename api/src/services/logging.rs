use std::fs::OpenOptions;
use std::io::Write;

use serde_json::{json, Value};

pub fn write_logs(std_err:&str, std_out:&str, file_name:&str) {
    // Create a file with name as filename_err.log and write the std_err log to the file
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(format!("{}_err.log", file_name))
        .unwrap();
    file.write_all(std_err.as_bytes()).unwrap();

    // Create a file with name as filename_out.log and write the std_out log to the file
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(format!("{}_out.log", file_name))
        .unwrap();
    file.write_all(std_out.as_bytes()).unwrap();
}

pub fn read_logs(file_name:&str) -> Value {
    // Read the contents of the file with name as filename_err.log
    let std_err = std::fs::read_to_string(format!("{}_err.log", file_name)).unwrap();

    // Read the contents of the file with name as filename_out.log
    let std_out = std::fs::read_to_string(format!("{}_out.log", file_name)).unwrap();

    // Return the logs as a JSON object
    json!({
        "std_err": std_err,
        "std_out": std_out,
    })
}