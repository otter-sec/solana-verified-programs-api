use serde::Deserialize;

/// Configuration for the API server. Loaded from env at startup (see
/// [`Config::from_env`]); request-time values are then carried in
/// [`crate::AppState`] so handlers/services don't have to read globals.
#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// PostgreSQL database URL
    pub database_url: String,
    /// RPC URL Note: get_program_accounts call should be enabled on the RPC node
    pub rpc_url: String,
    /// Auth secret
    pub auth_secret: String,
    /// Port to run the server on
    pub port: u16,
    /// Interval in seconds for the program_state sweep (default: 300 = 5 min)
    #[serde(default = "default_sweep_interval")]
    pub sweep_interval_seconds: u64,
    /// Maximum size of the Postgres connection pool (default: 50)
    #[serde(default = "default_db_max_connections")]
    pub db_max_connections: u32,
    /// Max re-verification builds the sweep will kick off in a single
    /// cycle (default: 3). Bounds the burst when many programs drift at
    /// once (e.g. first sweep after a deploy); the rest are picked up on
    /// later cycles.
    #[serde(default = "default_max_reverifies_per_sweep")]
    pub max_reverifies_per_sweep: usize,
}

impl Config {
    /// Reads `.env` (if present) and parses the env into a `Config`.
    pub fn from_env() -> Result<Self, envy::Error> {
        dotenvy::dotenv().ok();
        envy::from_env::<Self>()
    }
}

/// Default sweep interval: 5 minutes
fn default_sweep_interval() -> u64 {
    300
}

/// Default pool size: 50 connections
fn default_db_max_connections() -> u32 {
    50
}

/// Default per-sweep re-verification cap: 3 builds
fn default_max_reverifies_per_sweep() -> usize {
    3
}
