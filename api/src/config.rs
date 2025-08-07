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
    /// Interval in seconds for updating program status in background (default: 3600 = 1 hour)
    #[serde(default = "default_program_status_update_interval")]
    pub program_status_update_interval_seconds: u64,
    /// Batch size for program status updates (default: 100)
    #[serde(default = "default_program_status_batch_size")]
    pub program_status_batch_size: usize,
    /// Maximum concurrent RPC calls for program status updates (default: 20)
    #[serde(default = "default_program_status_max_concurrent")]
    pub program_status_max_concurrent: usize,
}

/// Default update interval: 1 hour
fn default_program_status_update_interval() -> u64 {
    3600
}

/// Default batch size: 100 programs per batch
fn default_program_status_batch_size() -> usize {
    100
}

/// Default max concurrent: 20 RPC calls
fn default_program_status_max_concurrent() -> usize {
    20
}
