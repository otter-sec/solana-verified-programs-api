use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Validates Solana public key
pub fn validate_pubkey(value: &str) -> Result<Pubkey, String> {
    if value.trim().is_empty() {
        return Err("Public key cannot be empty".to_string());
    }
    Pubkey::from_str(value).map_err(|e| format!("Invalid public key({}): {}", value, e))
}

/// Validates HTTP/HTTPS URL
pub fn validate_http_url(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("URL cannot be empty".to_string());
    }
    let url = reqwest::Url::parse(value).map_err(|e| format!("Invalid URL: {}", e))?;
    let host = url.host_str().filter(|h| !h.is_empty());
    if let Some(host) = host {
        const LOCALHOST_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "::1"];

        match url.scheme() {
            "https" => {}
            "http" => {
                let is_localhost = LOCALHOST_HOSTS.contains(&host) || host.starts_with("127.");
                if !is_localhost {
                    return Err("URL must use https except for localhost".to_string());
                }
            }
            _ => return Err("URL must use http or https scheme".to_string()),
        }
    } else {
        return Err("URL must have a valid host".to_string());
    }

    Ok(())
}

const SEARCH_VALIDATION_MSG: &str = "Search must be a valid Solana address or a valid URL";

/// Validates search query; must be a valid Solana public key or a valid URL
pub fn validate_search(search: &str) -> Result<(), String> {
    let s = search.trim();
    if s.is_empty() {
        return Ok(());
    }
    if validate_pubkey(s).is_ok() {
        return Ok(());
    }
    if validate_http_url(s).is_ok() {
        return Ok(());
    }
    Err(SEARCH_VALIDATION_MSG.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_pubkey() {
        assert_eq!(
            validate_pubkey("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC"),
            Ok(Pubkey::from_str("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC").unwrap())
        );
        assert_eq!(
            validate_pubkey(""),
            Err("Public key cannot be empty".to_string())
        );
        assert_eq!(
            validate_pubkey("12345678901234567890123456789012345678901"),
            Err("Invalid public key: Invalid Base58 string".to_string())
        );
    }

    #[test]
    fn test_validate_http_url() {
        assert_eq!(
            validate_http_url("https://github.com/solana-labs/solana"),
            Ok(())
        );
        assert_eq!(
            validate_http_url("http://github.com/solana-labs/solana"),
            Err("URL must use https except for localhost".to_string())
        );
        assert_eq!(validate_http_url("http://localhost:3000/callback"), Ok(()));
        assert_eq!(validate_http_url("http://127.0.0.1/callback"), Ok(()));
        assert_eq!(
            validate_http_url("ftp://github.com/solana-labs/solana"),
            Err("URL must use http or https scheme".to_string())
        );
        assert_eq!(
            validate_http_url("github.com/solana-labs/solana"),
            Err("Invalid URL: relative URL without a base".to_string())
        );
        assert_eq!(
            validate_http_url(""),
            Err("URL cannot be empty".to_string())
        );
    }

    #[test]
    fn test_validate_search() {
        assert_eq!(validate_search(""), Ok(()));
        assert_eq!(validate_search("   "), Ok(()));
        assert_eq!(
            validate_search("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC"),
            Ok(())
        );
        assert_eq!(validate_search("https://github.com/foo/bar"), Ok(()));
        assert_eq!(
            validate_search("not-a-pubkey-or-url"),
            Err(SEARCH_VALIDATION_MSG.to_string())
        );
        assert_eq!(
            validate_search("ftp://example.com"),
            Err(SEARCH_VALIDATION_MSG.to_string())
        );
    }
}
