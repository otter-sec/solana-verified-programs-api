pub fn get_last_line(output: &str) -> Option<String> {
    output.lines().last().map(ToOwned::to_owned)
}

pub fn extract_hash(output: &str, prefix: &str) -> Option<String> {
    output
        .lines()
        .find(|line| line.starts_with(prefix))
        .map(|line| {
            let hash = line.trim_start_matches(prefix.trim()).trim();
            hash.to_owned()
        })
}
