//! Request-time application state. Carried as `axum::extract::State`,
//! plumbed through services that need RPC access. Replaces the old
//! global `CONFIG` so tests can construct an app without env vars.

use crate::db::DbClient;
use axum::extract::FromRef;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DbClient,
    pub rpc: Arc<RpcClient>,
    /// Used by `solana-verify` invocations + log redaction.
    pub rpc_url: Arc<str>,
    /// Compared (constant-time) against the `Authorization` header on
    /// webhook endpoints.
    pub auth_secret: Arc<str>,
    /// Configured sweep cadence; the `/health/background-jobs` handler
    /// uses it to decide whether the last sweep is recent enough.
    pub sweep_interval_seconds: u64,
    /// Max re-verification builds the sweep kicks per cycle.
    pub max_reverifies_per_sweep: usize,
}

impl AppState {
    pub fn new(
        db: DbClient,
        rpc_url: &str,
        auth_secret: &str,
        sweep_interval_seconds: u64,
        max_reverifies_per_sweep: usize,
    ) -> Self {
        Self {
            db,
            rpc: Arc::new(RpcClient::new(rpc_url.to_string())),
            rpc_url: rpc_url.into(),
            auth_secret: auth_secret.into(),
            sweep_interval_seconds,
            max_reverifies_per_sweep,
        }
    }
}

/// Lets handlers that only need the DB keep `State<DbClient>` instead of
/// reaching into `AppState`.
impl FromRef<AppState> for DbClient {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}
