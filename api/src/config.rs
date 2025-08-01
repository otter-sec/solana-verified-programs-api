use serde::Deserialize;

/// Configuration for the API server
#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// PostgreSQL database URL
    pub database_url: String,
    /// Redis URL
    pub redis_url: String,
    /// RPC URL Note: get_program_accounts call should be enabled on the RPC node
    pub rpc_url: String,
    /// List of RPC URLs for key rotation (comma-separated)
    /// If not provided, falls back to rpc_url
    pub rpc_urls: Option<String>,
    /// Auth secret
    pub auth_secret: String,
    /// Port to run the server on
    pub port: u16,
}
