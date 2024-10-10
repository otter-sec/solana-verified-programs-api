use std::fs::OpenOptions;
use std::io::Write;


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