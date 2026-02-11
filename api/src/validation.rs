use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Validates Solana public key
pub fn validate_pubkey(value: &str) -> Result<Pubkey, String> {
    if value.trim().is_empty() {
        return Err("Public key cannot be empty".to_string());
    }
    Pubkey::from_str(value).map_err(|e| format!("Invalid public key: {}", e))
}

/// Validates HTTP/HTTPS URL
pub fn validate_http_url(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("URL cannot be empty".to_string());
    }
    let url = url::Url::parse(value).map_err(|e| format!("Invalid URL: {}", e))?;
    match url.scheme() {
        "http" | "https" => {}
        _ => return Err("URL must use http or https scheme".to_string()),
    }
    if url.host_str().filter(|h| !h.is_empty()).is_none() {
        return Err("URL must have a valid host".to_string());
    }
    Ok(())
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
            Err(
                "Invalid public key: Invalid Base58 string"
                    .to_string()
            )
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
            Ok(())
        );
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
}
