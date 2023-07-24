pub fn get_last_line(output: &str) -> Option<String> {
    output.lines().last().map(|line| line.to_owned())
}

pub fn extract_hash(output: &str, prefix: &str) -> Option<String> {
    if let Some(line) = output.lines().find(|line| line.starts_with(prefix)) {
        let hash = line.trim_start_matches(prefix.trim()).trim();
        Some(hash.to_owned())
    } else {
        None
    }
}
