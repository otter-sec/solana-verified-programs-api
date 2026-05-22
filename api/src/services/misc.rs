/// Constructs a repository URL with optional commit hash
///
/// Trailing `/` and a `.git` suffix are stripped from the base URL so that the
/// resulting links are normalized regardless of how the repository was provided.
///
/// # Arguments
/// * `repository` - Base repository URL
/// * `commit` - Optional commit hash (or `"None"`, which is treated as absent)
///
/// # Returns
/// * `String` - Full repository URL, optionally including commit reference
pub fn build_repository_url(repository: &str, commit: Option<&str>) -> String {
    let trimmed = repository.trim_end_matches('/');
    let repository = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    if let Some(hash) = commit {
        if !hash.is_empty() && hash != "None" {
            return format!("{repository}/tree/{hash}");
        }
    }
    repository.to_string()
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
    fn test_build_repository_url() {
        assert_eq!(
            build_repository_url("https://github.com/user/repo/", Some("abc123")),
            "https://github.com/user/repo/tree/abc123"
        );
        assert_eq!(
            build_repository_url("https://github.com/user/repo/", None),
            "https://github.com/user/repo"
        );
        assert_eq!(
            build_repository_url("https://github.com/user/repo/", Some("")),
            "https://github.com/user/repo"
        );
        // A `.git` suffix is stripped, with or without a trailing slash.
        assert_eq!(
            build_repository_url("https://github.com/user/repo.git", Some("abc123")),
            "https://github.com/user/repo/tree/abc123"
        );
        assert_eq!(
            build_repository_url("https://github.com/user/repo.git", None),
            "https://github.com/user/repo"
        );
        assert_eq!(
            build_repository_url("https://github.com/user/repo.git/", Some("abc123")),
            "https://github.com/user/repo/tree/abc123"
        );
    }

    #[test]
    fn test_extract_hash_with_prefix() {
        let output = "Program Hash: abc123\nRandom text";
        let hash = extract_hash_with_prefix(output, "Program Hash:");
        assert_eq!(hash, Some("abc123".to_string()));
    }
}
