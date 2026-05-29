//! Periodic refresh of `program_state` rows -- the slow path; webhooks are
//! the fast path. Each cycle covers every row in `program_state` via batched
//! `getMultipleAccounts` calls, and re-verifies (up to a per-cycle cap) any
//! program whose on-chain hash drifted since the last sweep -- the backstop
//! for upgrades the `/pda` webhook missed.

use crate::{
    db::{DbClient, NewBuild},
    services::onchain::{get_otter_verify_params, snapshot_programs},
    services::verification,
    state::AppState,
    validation::Address,
};
use solana_pubkey::Pubkey;
use std::{str::FromStr, time::Duration};
use tracing::{error, info};

/// Spawns the sweep task. Runs for the process's lifetime.
pub fn spawn(state: AppState) {
    let interval_seconds = state.sweep_interval_seconds;
    tokio::spawn(async move {
        let interval = Duration::from_secs(interval_seconds);
        let mut ticker = tokio::time::interval(interval);
        // If a sweep cycle outlasts the interval, don't fire catch-up ticks
        // back-to-back -- wait the full interval after it finishes.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        info!("sweep loop started, interval={interval_seconds}s");
        loop {
            ticker.tick().await;
            if let Err(e) = run_once(&state).await {
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

async fn run_once(state: &AppState) -> crate::errors::Result<()> {
    let db = &state.db;
    let ids = db.sweep_program_ids().await?;
    if ids.is_empty() {
        return Ok(());
    }
    let pubkeys: Vec<Pubkey> = ids
        .iter()
        .filter_map(|s| Pubkey::from_str(s).ok())
        .collect();
    info!("sweep: refreshing {} programs", pubkeys.len());

    // `upsert_program_state` sets `pending_reverify` whenever a program's
    // hash drifts; the drain below consumes that queue.
    let snapshots = snapshot_programs(&state.rpc, &pubkeys).await?;
    for (pid, snap) in &snapshots {
        if let Err(e) = db.upsert_program_state(&Address(*pid), snap).await {
            error!("upsert state for {}: {}", pid, e);
        }
    }
    info!("sweep: applied {} snapshots", snapshots.len());

    drain_reverify_queue(state).await;
    Ok(())
}

/// Drains up to `max_reverifies_per_sweep` drift-flagged programs. Overflow
/// stays flagged for the next cycle. The flag is cleared per program once
/// handled, so a still-broken program isn't re-examined until it drifts again.
async fn drain_reverify_queue(state: &AppState) {
    let cap = state.max_reverifies_per_sweep as i64;
    let candidates = match state.db.pending_reverify_candidates(cap).await {
        Ok(c) => c,
        Err(e) => {
            error!("sweep: load reverify candidates: {}", e);
            return;
        }
    };

    for (program_id, authority) in candidates {
        if let Err(e) = reverify_one(state, &program_id, authority).await {
            error!("sweep: reverify {}: {}", program_id, e);
        }
        // Clear regardless of outcome -- only a fresh drift re-queues it.
        if let Err(e) = state.db.clear_pending_reverify(&program_id).await {
            error!("sweep: clear pending for {}: {}", program_id, e);
        }
    }
}

/// Fetches the program's current Otter Verify PDA and, unless an identical
/// build already exists (any status -- so failures aren't retried), kicks a
/// fresh verify through the same `execute` path as the verify endpoints.
async fn reverify_one(
    state: &AppState,
    program_id: &Address,
    authority: Option<String>,
) -> crate::errors::Result<()> {
    // signer=None -> tries the authority, then the whitelisted SIGNER_KEYS.
    let (otter_params, _) =
        match get_otter_verify_params(&state.rpc, &program_id.to_string(), None, authority).await {
            Ok(v) => v,
            Err(e) => {
                // No trusted PDA to build from -- nothing to do, not an error.
                info!("sweep: no usable PDA for {}, skipping: {}", program_id, e);
                return Ok(());
            }
        };

    let new_build = NewBuild::from(&otter_params);

    // Skip if these params were already built (any status, incl. failed):
    // covers the unchanged-PDA case and avoids retrying failures.
    if state.db.has_build_for_params(&new_build).await? {
        return Ok(());
    }

    let build_id = state.db.insert_build(&new_build).await?;
    info!("sweep: re-verifying {} (build {})", program_id, build_id);
    let task_state = state.clone();
    tokio::spawn(async move {
        verification::execute(build_id, new_build, task_state, None).await;
    });
    Ok(())
}
