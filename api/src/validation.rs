use serde::{Deserialize, Deserializer, Serialize, Serializer};
use solana_pubkey::Pubkey;
use std::fmt;
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

// --- Newtypes ----------------------------------------------------------------
//
// Strongly-typed wrappers for fields that come from the request boundary.
// `FromStr` does the validation; `Deserialize` plumbs it into serde so
// request bodies and path extractors fail with 422 on bad input rather
// than silently accepting it.

fn deserialize_via_from_str<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr<Err = String>,
{
    let s = String::deserialize(d)?;
    T::from_str(&s).map_err(serde::de::Error::custom)
}

/// Validated Solana address. Used for program IDs, PDA signers, and any
/// other request-boundary pubkey field. Serializes as the base58 string;
/// stored in postgres as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Address(pub Pubkey);

impl Address {
    pub fn as_pubkey(&self) -> &Pubkey {
        &self.0
    }
}

impl FromStr for Address {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_pubkey(s).map(Address)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for Address {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        deserialize_via_from_str(d)
    }
}

impl sqlx::Type<sqlx::Postgres> for Address {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }
    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for Address {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        self.0.to_string().encode_by_ref(buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for Address {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Self::from_str(&s).map_err(Into::into)
    }
}

/// URL the API will POST verification results to. `https://` only,
/// except `http://` is allowed for loopback hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookUrl(String);

impl WebhookUrl {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl FromStr for WebhookUrl {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_http_url(s)?;
        Ok(WebhookUrl(s.trim().to_string()))
    }
}

impl fmt::Display for WebhookUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for WebhookUrl {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for WebhookUrl {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        deserialize_via_from_str(d)
    }
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
        assert!(validate_pubkey("12345678901234567890123456789012345678901").is_err());
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
        assert!(validate_http_url("github.com/solana-labs/solana").is_err());
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

    #[test]
    fn program_id_parses_via_serde() {
        let p: Address =
            serde_json::from_str("\"verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC\"").unwrap();
        assert_eq!(p.to_string(), "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC");
        assert!(serde_json::from_str::<Address>("\"\"").is_err());
        assert!(serde_json::from_str::<Address>("\"bad\"").is_err());
    }

    #[test]
    fn webhook_url_rejects_plain_http() {
        assert!(WebhookUrl::from_str("http://example.com/cb").is_err());
        assert!(WebhookUrl::from_str("https://example.com/cb").is_ok());
        assert!(WebhookUrl::from_str("http://127.0.0.1/cb").is_ok());
    }
}
