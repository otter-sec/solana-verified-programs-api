//! Periodic refresh of `program_state` rows -- the slow path; webhooks are
//! the fast path. Each cycle covers every row in `program_state` via batched
//! `getMultipleAccounts` calls.

use crate::{db::DbClient, services::onchain::snapshot_programs, validation::Address};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use std::{str::FromStr, sync::Arc, time::Duration};
use tracing::{error, info};

/// Spawns the sweep task. Runs for the process's lifetime.
pub fn spawn(db: DbClient, rpc: Arc<RpcClient>, interval_seconds: u64) {
    tokio::spawn(async move {
        let interval = Duration::from_secs(interval_seconds);
        let mut ticker = tokio::time::interval(interval);
        // If a sweep cycle outlasts the interval, don't fire catch-up ticks
        // back-to-back -- wait the full interval after it finishes.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        info!("sweep loop started, interval={interval_seconds}s");
        loop {
            ticker.tick().await;
            if let Err(e) = run_once(&db, &rpc).await {
                error!("sweep cycle: {}", e);
            }
        }
    });
}

/// Health view for the `/health/background-jobs` endpoint, derived from
/// the timestamp on the oldest `program_state` row.
pub struct BackgroundJobManager<'a> {
    db: &'a DbClient,
    sweep_interval_seconds: u64,
}

impl<'a> BackgroundJobManager<'a> {
    pub fn new(db: &'a DbClient, sweep_interval_seconds: u64) -> Self {
        Self {
            db,
            sweep_interval_seconds,
        }
    }

    pub async fn get_health_status(&self) -> crate::responses::BackgroundJobHealth {
        use crate::responses::{BackgroundJobHealth, BackgroundJobStatus};
        let last = self.db.last_sweep_at().await.ok().flatten();
        let now = chrono::Utc::now();
        let interval = chrono::Duration::seconds(self.sweep_interval_seconds as i64);
        match last {
            Some(t) => {
                let lag = now - t;
                if lag > interval * 2 {
                    BackgroundJobHealth {
                        status: BackgroundJobStatus::Inactive,
                        last_program_check: Some(t.naive_utc()),
                        message: format!(
                            "Last sweep was {}s ago, expected interval {}s",
                            lag.num_seconds(),
                            interval.num_seconds()
                        ),
                    }
                } else {
                    BackgroundJobHealth {
                        status: BackgroundJobStatus::Active,
                        last_program_check: Some(t.naive_utc()),
                        message: "Background sweep running normally".into(),
                    }
                }
            }
            None => BackgroundJobHealth {
                status: BackgroundJobStatus::Unknown,
                last_program_check: None,
                message: "no program_state rows yet".into(),
            },
        }
    }
}

async fn run_once(db: &DbClient, rpc: &RpcClient) -> crate::errors::Result<()> {
    let ids = db.sweep_program_ids().await?;
    if ids.is_empty() {
        return Ok(());
    }
    let pubkeys: Vec<Pubkey> = ids
        .iter()
        .filter_map(|s| Pubkey::from_str(s).ok())
        .collect();
    info!("sweep: refreshing {} programs", pubkeys.len());

    let snapshots = snapshot_programs(rpc, &pubkeys).await?;
    for (pid, snap) in &snapshots {
        let program_id = Address(*pid);
        if let Err(e) = db.upsert_program_state(&program_id, snap).await {
            error!("upsert state for {}: {}", program_id, e);
        }
    }
    info!("sweep: applied {} snapshots", snapshots.len());
    Ok(())
}
