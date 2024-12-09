use crate::db::models::SolanaProgramBuild;

/// Extracts the last line from a multi-line string
///
/// # Arguments
/// * `output` - Multi-line string to process
///
/// # Returns
/// * `Option<String>` - Last line if present, None if empty
pub fn get_last_line(output: &str) -> Option<String> {
    output.lines().last().map(ToOwned::to_owned)
}

/// Constructs a repository URL with optional commit hash
///
/// # Arguments
/// * `build_params` - Build parameters containing repository and commit information
///
/// # Returns
/// * `String` - Full repository URL, optionally including commit reference
pub fn build_repository_url(build_params: &SolanaProgramBuild) -> String {
    if let Some(hash) = &build_params.commit_hash {
        if !hash.is_empty() {
            return format!(
                "{}/tree/{}",
                build_params.repository.trim_end_matches('/'),
                hash
            );
        }
    }
    build_params.repository.clone()
}

/// Extracts a hash value from output text with a specific prefix
///
/// # Arguments
/// * `output` - Text to search through
/// * `prefix` - Prefix string that precedes the hash
///
/// # Returns
/// * `Option<String>` - Extracted hash if found
///
/// # Example
/// ```
/// let output = "Program Hash: abc123\nOther text";
/// let hash = extract_hash_with_prefix(output, "Program Hash:");
/// assert_eq!(hash, Some("abc123".to_string()));
/// ```
pub fn extract_hash_with_prefix(output: &str, prefix: &str) -> Option<String> {
    output
        .lines()
        .find(|line| line.starts_with(prefix))
        .map(|line| {
            let hash = line.trim_start_matches(prefix.trim()).trim();
            hash.to_owned()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_last_line() {
        assert_eq!(get_last_line("Hello\nWorld"), Some("World".to_string()));
        assert_eq!(get_last_line(""), None);
        assert_eq!(get_last_line("Solana"), Some("Solana".to_string()));
    }

    #[test]
    fn test_get_repo_url() {
        let mut build = SolanaProgramBuild {
            repository: "https://github.com/user/repo/".to_string(),
            commit_hash: Some("abc123".to_string()),
            ..Default::default()
        };
        assert_eq!(
            build_repository_url(&build),
            "https://github.com/user/repo/tree/abc123"
        );

        build.commit_hash = None;
        assert_eq!(build_repository_url(&build), "https://github.com/user/repo/");
        
        build.commit_hash = Some("".to_string());
        assert_eq!(build_repository_url(&build), "https://github.com/user/repo/");
    }

    #[test]
    fn test_extract_hash_with_prefix() {
        let output = "Program Hash: abc123\nRandom text";
        let hash = extract_hash_with_prefix(output, "Program Hash:");
        assert_eq!(hash, Some("abc123".to_string()));
    }
}
