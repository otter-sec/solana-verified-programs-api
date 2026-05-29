//! Postgres data layer. Two tables of substance: `builds` (one row per
//! verification attempt -- job + result merged) and `program_state` (one
//! cached row per program). Queries are runtime-checked -- `sqlx::query`
//! (no `!`) + `FromRow` derive -- so `SELECT *` works and there's no
//! offline cache to keep in sync with migrations.

use crate::{
    errors::{ApiError, Result},
    services::onchain::ProgramOnchainState,
    validation::Address,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

/// Lifecycle state of a verification job. Backed by the postgres
/// `build_status` ENUM type (see `migrations/0001_init.sql`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "build_status", rename_all = "snake_case")]
pub enum JobStatus {
    InProgress,
    Completed,
    Failed,
}

pub const PER_PAGE: i64 = 20;

#[derive(Clone)]
pub struct DbClient {
    pool: PgPool,
    /// Cached `/status` response bodies, keyed by program. Every mutating
    /// path (`upsert_program_state`, `unverify_program`, `mark_closed`,
    /// `mark_build_completed`) invalidates the entry; the TTL is just a
    /// safety net for missed invalidations.
    verify_cache: moka::future::Cache<Address, String>,
}

impl DbClient {
    /// Opens a bounded connection pool against `url`. `verify_cache_ttl`
    /// should be `sweep_interval_seconds`: every sweep cycle upserts every
    /// `program_state` row, which invalidates the matching cache entry, so
    /// a longer TTL would never fire and a shorter one just adds DB load
    /// between sweeps.
    pub async fn connect(
        url: &str,
        max_connections: u32,
        verify_cache_ttl: Duration,
    ) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(30))
            .connect(url)
            .await?;
        let verify_cache = moka::future::Cache::builder()
            .max_capacity(10_000)
            .time_to_live(verify_cache_ttl)
            .build();
        Ok(Self { pool, verify_cache })
    }

    /// Runs all pending embedded migrations.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| ApiError::Custom(format!("migration: {e}")))?;
        info!("migrations applied");
        Ok(())
    }

    /// `SELECT 1` for the health endpoint.
    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct BuildRow {
    pub id: Uuid,
    pub repository: String,
    pub commit_hash: Option<String>,
    pub program_id: Address,
    pub lib_name: Option<String>,
    pub base_docker_image: Option<String>,
    pub mount_path: Option<String>,
    pub cargo_args: Option<Vec<String>>,
    pub bpf_flag: bool,
    pub arch: Option<String>,
    pub signer: Option<Address>,
    pub status: JobStatus,
    pub executable_hash: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Subset of `program_state` callers actually read. `authority` and
/// `last_checked` exist on the row but aren't surfaced anywhere yet.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProgramStateRow {
    pub on_chain_hash: Option<String>,
    pub is_frozen: bool,
    pub is_closed: bool,
}

/// Projection of just the columns `check_is_verified` reads to build a
/// response. Both sides of the LATERAL join can miss, so everything is
/// nullable.
#[derive(sqlx::FromRow)]
struct VerificationRow {
    on_chain_hash: Option<String>,
    is_frozen: Option<bool>,
    is_closed: Option<bool>,
    executable_hash: Option<String>,
    repository: Option<String>,
    commit_hash: Option<String>,
    completed_at: Option<DateTime<Utc>>,
}

/// Identifying parameters for a build, before insertion.
#[derive(Debug, Clone)]
pub struct NewBuild {
    pub repository: String,
    pub commit_hash: Option<String>,
    pub program_id: Address,
    pub lib_name: Option<String>,
    pub base_docker_image: Option<String>,
    pub mount_path: Option<String>,
    pub cargo_args: Option<Vec<String>>,
    /// Passed to `solana-verify` as `--cargo-build-sbf-args=...`. Sourced from
    /// the on-chain PDA only; not persisted, so it's `None` on re-verification.
    pub cargo_build_sbf_args: Option<String>,
    pub bpf_flag: bool,
    pub arch: Option<String>,
    pub signer: Option<Address>,
}

impl From<&crate::services::onchain::OtterBuildParams> for NewBuild {
    fn from(p: &crate::services::onchain::OtterBuildParams) -> Self {
        NewBuild {
            repository: p.git_url.clone(),
            commit_hash: Some(p.commit.clone()),
            program_id: Address(p.address),
            lib_name: p.get_library_name(),
            base_docker_image: p.get_base_image(),
            mount_path: p.get_mount_path(),
            cargo_args: p.get_cargo_args(),
            cargo_build_sbf_args: p.get_cargo_build_sbf_args(),
            bpf_flag: p.is_bpf(),
            arch: p.get_arch(),
            signer: Some(Address(p.signer)),
        }
    }
}

impl DbClient {
    /// Inserts an `in_progress` build row and returns its UUID.
    pub async fn insert_build(&self, b: &NewBuild) -> Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO builds (
                id, repository, commit_hash, program_id, lib_name,
                base_docker_image, mount_path, cargo_args, bpf_flag, arch,
                signer, status
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        )
        .bind(id)
        .bind(&b.repository)
        .bind(&b.commit_hash)
        .bind(b.program_id)
        .bind(&b.lib_name)
        .bind(&b.base_docker_image)
        .bind(&b.mount_path)
        .bind(&b.cargo_args)
        .bind(b.bpf_flag)
        .bind(&b.arch)
        .bind(b.signer)
        .bind(JobStatus::InProgress)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// Transitions a build to `completed`, records its executable hash, and
    /// invalidates the `program_id`'s cached `/status` response.
    pub async fn mark_build_completed(
        &self,
        id: Uuid,
        program_id: &Address,
        executable_hash: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE builds SET status = $1, executable_hash = $2, completed_at = NOW() WHERE id = $3",
        )
        .bind(JobStatus::Completed)
        .bind(executable_hash)
        .bind(id)
        .execute(&self.pool)
        .await?;
        self.verify_cache.invalidate(program_id).await;
        Ok(())
    }

    /// Transitions a build to `failed` with the given error message.
    pub async fn mark_build_failed(&self, id: Uuid, error: &str) -> Result<()> {
        sqlx::query(
            "UPDATE builds SET status = $1, error_message = $2, completed_at = NOW() WHERE id = $3",
        )
        .bind(JobStatus::Failed)
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Marks every `in_progress` row as `failed`. Called at startup so
    /// builds whose owning task died with a previous process don't sit
    /// blocking the dedupe filter forever. Returns the number of rows
    /// flipped.
    pub async fn fail_orphan_builds(&self) -> Result<u64> {
        let rows = sqlx::query(
            "UPDATE builds
             SET status = 'failed',
                 error_message = 'orphaned by server restart',
                 completed_at = NOW()
             WHERE status = 'in_progress'",
        )
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(rows)
    }

    /// Fetches a build by id.
    pub async fn get_build(&self, id: Uuid) -> Result<Option<BuildRow>> {
        Ok(
            sqlx::query_as::<_, BuildRow>("SELECT * FROM builds WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// Most recent build with identical params. `include_failed = false`
    /// ignores failed rows (they're retryable); `true` counts every status.
    /// `$11` toggles the failed filter so both callers share one query.
    async fn latest_build_for_params(
        &self,
        b: &NewBuild,
        include_failed: bool,
    ) -> Result<Option<BuildRow>> {
        Ok(sqlx::query_as::<_, BuildRow>(
            "SELECT * FROM builds
             WHERE program_id = $1
               AND repository = $2
               AND (commit_hash       IS NOT DISTINCT FROM $3)
               AND (lib_name          IS NOT DISTINCT FROM $4)
               AND (base_docker_image IS NOT DISTINCT FROM $5)
               AND (mount_path        IS NOT DISTINCT FROM $6)
               AND (cargo_args        IS NOT DISTINCT FROM $7)
               AND bpf_flag = $8
               AND (arch              IS NOT DISTINCT FROM $9)
               AND (signer            IS NOT DISTINCT FROM $10)
               AND ($11 OR status <> 'failed')
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(b.program_id)
        .bind(&b.repository)
        .bind(&b.commit_hash)
        .bind(&b.lib_name)
        .bind(&b.base_docker_image)
        .bind(&b.mount_path)
        .bind(&b.cargo_args)
        .bind(b.bpf_flag)
        .bind(&b.arch)
        .bind(b.signer)
        .bind(include_failed)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Most recent non-failed build with identical params; failed rows are
    /// ignored (they're retryable).
    pub async fn find_duplicate(&self, b: &NewBuild) -> Result<Option<BuildRow>> {
        self.latest_build_for_params(b, false).await
    }

    /// Whether *any* build with identical params exists, counting `failed`
    /// rows -- so a failed attempt blocks a rebuild rather than retrying.
    pub async fn has_build_for_params(&self, b: &NewBuild) -> Result<bool> {
        Ok(self.latest_build_for_params(b, true).await?.is_some())
    }

    /// Up to `limit` programs the sweep flagged as drifted (`pending_reverify`)
    /// and that are still buildable. Oldest-checked first, so a capped burst
    /// drains over successive cycles. Returns `(program_id, authority)` --
    /// authority seeds the Otter Verify PDA lookup.
    pub async fn pending_reverify_candidates(
        &self,
        limit: i64,
    ) -> Result<Vec<(Address, Option<String>)>> {
        Ok(sqlx::query_as(
            "SELECT program_id, authority FROM program_state
             WHERE pending_reverify AND NOT is_closed AND NOT is_frozen
             ORDER BY last_checked ASC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Clears the `pending_reverify` flag once the sweep has acted on a
    /// program (kicked a build, or decided there was nothing to do). It only
    /// comes back via a fresh drift in [`upsert_program_state`].
    pub async fn clear_pending_reverify(&self, program_id: &Address) -> Result<()> {
        sqlx::query("UPDATE program_state SET pending_reverify = FALSE WHERE program_id = $1")
            .bind(program_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// One row per signer who has a completed claim on this program.
    pub async fn get_all_verification_info(
        &self,
        program_id: Address,
    ) -> Result<Vec<crate::responses::VerificationResponseWithSigner>> {
        let state = self.get_program_state(&program_id).await?;
        let builds = sqlx::query_as::<_, BuildRow>(
            "SELECT DISTINCT ON (signer) * FROM builds
             WHERE program_id = $1 AND status = 'completed'
             ORDER BY signer, completed_at DESC",
        )
        .bind(program_id)
        .fetch_all(&self.pool)
        .await?;

        use crate::responses::{VerificationResponse, VerificationResponseWithSigner};
        Ok(builds
            .into_iter()
            .map(|b| VerificationResponseWithSigner {
                signer: b.signer,
                verification_response: VerificationResponse::from_state_and_build(
                    state.as_ref(),
                    Some(&b),
                ),
            })
            .collect())
    }

    /// Serialized `GET /status/{program_id}` response body. Joins
    /// `program_state` (cached on-chain hash + frozen/closed flags) with the
    /// best matching completed build in a single `LEFT JOIN LATERAL`, then
    /// renders an `ExtendedStatusResponse` directly to JSON. Result cached
    /// in-process; cache invalidated on every write that affects the row.
    ///
    /// Builds are restricted to a trusted signer: `SIGNER_KEYS`,
    /// `system_program::ID`, the program's current upgrade authority
    /// (matched live via `program_state.authority`), or NULL. Without
    /// this filter, an untrusted signer with a build that reproduces
    /// the on-chain hash could surface its own `repo_url` / `commit`.
    pub async fn check_is_verified(&self, program_id: Address) -> Result<String> {
        self.verify_cache
            .try_get_with(program_id, async move {
                let trusted = crate::services::onchain::trusted_signers();
                let row: VerificationRow = sqlx::query_as(
                    "SELECT ps.on_chain_hash, ps.is_frozen, ps.is_closed,
                            b.executable_hash, b.repository, b.commit_hash, b.completed_at
                     FROM (VALUES ($1::text)) AS v(program_id)
                     LEFT JOIN program_state ps ON ps.program_id = v.program_id
                     LEFT JOIN LATERAL (
                         SELECT executable_hash, repository, commit_hash, completed_at
                         FROM builds
                         WHERE program_id = v.program_id AND status = 'completed'
                           AND (signer IS NULL
                                OR signer = ANY($2)
                                OR signer IS NOT DISTINCT FROM ps.authority)
                         ORDER BY (executable_hash IS NOT DISTINCT FROM ps.on_chain_hash) DESC,
                                  completed_at DESC
                         LIMIT 1
                     ) b ON TRUE",
                )
                .bind(program_id)
                .bind(&trusted)
                .fetch_one(&self.pool)
                .await?;

                let on_chain_hash = row.on_chain_hash.unwrap_or_default();
                let is_closed = row.is_closed.unwrap_or(false);
                let is_verified = !on_chain_hash.is_empty()
                    && row.executable_hash.as_deref() == Some(on_chain_hash.as_str())
                    && !is_closed;
                let message = if is_verified {
                    "On chain program verified"
                } else {
                    "On chain program not verified"
                };
                let response = crate::responses::ExtendedStatusResponse {
                    status: crate::responses::StatusResponse {
                        is_verified,
                        message: message.to_string(),
                        on_chain_hash,
                        executable_hash: row.executable_hash.unwrap_or_default(),
                        repo_url: row
                            .repository
                            .as_deref()
                            .map(|r| {
                                crate::services::misc::build_repository_url(
                                    r,
                                    row.commit_hash.as_deref(),
                                )
                            })
                            .unwrap_or_default(),
                        commit: row.commit_hash.unwrap_or_default(),
                        last_verified_at: row.completed_at.map(|t| t.naive_utc()),
                    },
                    is_frozen: row.is_frozen.unwrap_or(false),
                    is_closed,
                };
                serde_json::to_string(&response)
                    .map_err(|e| ApiError::Custom(format!("encode /status body: {e}")))
            })
            .await
            .map_err(|e| ApiError::Custom(format!("check_is_verified: {e}")))
    }

    /// `program_state.on_chain_hash` for `program_id`, or "" when the row
    /// is absent / the column is NULL. Empty string is the sentinel callers
    /// compare against -- never a real hash, so any non-empty fresh value
    /// will compare unequal.
    pub async fn cached_on_chain_hash(&self, program_id: &Address) -> Result<String> {
        Ok(self
            .get_program_state(program_id)
            .await?
            .and_then(|s| s.on_chain_hash)
            .unwrap_or_default())
    }

    /// Cached on-chain state for a program.
    pub async fn get_program_state(&self, program_id: &Address) -> Result<Option<ProgramStateRow>> {
        Ok(sqlx::query_as::<_, ProgramStateRow>(
            "SELECT on_chain_hash, is_frozen, is_closed FROM program_state WHERE program_id = $1",
        )
        .bind(program_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Full refresh from a snapshot. A `None` hash on the snapshot preserves
    /// the existing column rather than clobbering it, so a transient hash
    /// fetch failure doesn't lose previously known data.
    ///
    /// Flips `pending_reverify` TRUE when the incoming hash actually differs
    /// from the stored one -- that's the sweep's drift signal, drained by
    /// [`pending_reverify_candidates`]. New rows leave it at its FALSE
    /// default, so a program's first sighting doesn't queue a reverify.
    pub async fn upsert_program_state(
        &self,
        program_id: &Address,
        state: &ProgramOnchainState,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO program_state
                (program_id, on_chain_hash, authority, is_frozen, is_closed, last_checked)
             VALUES ($1, $2, $3, $4, $5, NOW())
             ON CONFLICT (program_id) DO UPDATE
             SET on_chain_hash = COALESCE(EXCLUDED.on_chain_hash, program_state.on_chain_hash),
                 authority     = EXCLUDED.authority,
                 is_frozen     = EXCLUDED.is_frozen,
                 is_closed     = EXCLUDED.is_closed,
                 last_checked  = NOW(),
                 pending_reverify = program_state.pending_reverify
                     OR (EXCLUDED.on_chain_hash IS NOT NULL
                         AND EXCLUDED.on_chain_hash IS DISTINCT FROM program_state.on_chain_hash)",
        )
        .bind(program_id)
        .bind(&state.executable_hash)
        .bind(&state.authority)
        .bind(state.is_frozen)
        .bind(state.is_closed)
        .execute(&self.pool)
        .await?;
        self.verify_cache.invalidate(program_id).await;
        Ok(())
    }

    /// Updates the cached on-chain hash for a program after an upgrade, and
    /// sets `pending_reverify` directly: this advances the stored hash, so the
    /// sweep's own drift check wouldn't catch it -- the flag is the backstop.
    pub async fn unverify_program(&self, program_id: &Address, on_chain_hash: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO program_state (program_id, on_chain_hash, last_checked)
             VALUES ($1, $2, NOW())
             ON CONFLICT (program_id) DO UPDATE
             SET on_chain_hash = EXCLUDED.on_chain_hash,
                 last_checked = NOW(),
                 pending_reverify = TRUE",
        )
        .bind(program_id)
        .bind(on_chain_hash)
        .execute(&self.pool)
        .await?;
        self.verify_cache.invalidate(program_id).await;
        Ok(())
    }

    /// Records a program as closed and clears its authority.
    pub async fn mark_closed(&self, program_id: &Address) -> Result<()> {
        sqlx::query(
            "INSERT INTO program_state (program_id, is_closed, last_checked)
             VALUES ($1, TRUE, NOW())
             ON CONFLICT (program_id) DO UPDATE
             SET is_closed = TRUE, authority = NULL, last_checked = NOW()",
        )
        .bind(program_id)
        .execute(&self.pool)
        .await?;
        self.verify_cache.invalidate(program_id).await;
        Ok(())
    }

    /// One page of currently-verified program IDs plus the total count.
    /// `search` (empty disables filtering) is matched against both
    /// `program_id` and `repository`. `COUNT(*) OVER ()` gives the total
    /// in the same round-trip as the page.
    pub async fn get_verified_program_ids_page(
        &self,
        page: i64,
        search: Option<&str>,
    ) -> Result<(Vec<String>, i64)> {
        let page = page.max(1);
        let offset = (page - 1) * PER_PAGE;
        let search = search.unwrap_or("").trim();
        let pattern = format!("%{search}%");

        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT program_id, COUNT(*) OVER ()
             FROM (
                 SELECT DISTINCT b.program_id
                 FROM builds b
                 LEFT JOIN program_state ps ON ps.program_id = b.program_id
                 WHERE b.status = 'completed'
                   AND b.executable_hash IS NOT NULL
                   AND b.executable_hash = ps.on_chain_hash
                   AND NOT COALESCE(ps.is_closed, FALSE)
                   AND NOT COALESCE(ps.is_frozen, FALSE)
                   AND ($1 = '' OR b.program_id ILIKE $2 OR b.repository ILIKE $2)
             ) q
             ORDER BY program_id
             LIMIT $3 OFFSET $4",
        )
        .bind(search)
        .bind(&pattern)
        .bind(PER_PAGE)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let total = rows.first().map_or(0, |(_, n)| *n);
        let ids = rows.into_iter().map(|(id, _)| id).collect();
        Ok((ids, total))
    }

    /// Latest trusted-signer completed build for every currently-verified
    /// program. The `JOIN program_state` predicates and the signer filter
    /// enforce verified-ness in SQL, so each row maps straight to the response.
    pub async fn get_verification_status_all(
        &self,
    ) -> Result<Vec<crate::responses::VerifiedProgramStatusResponse>> {
        let trusted = crate::services::onchain::trusted_signers();
        let builds = sqlx::query_as::<_, BuildRow>(
            "SELECT DISTINCT ON (b.program_id) b.*
             FROM builds b
             JOIN program_state ps ON ps.program_id = b.program_id
               AND ps.on_chain_hash = b.executable_hash
               AND NOT ps.is_closed AND NOT ps.is_frozen
             WHERE b.status = 'completed'
               AND (b.signer IS NULL
                    OR b.signer = ANY($1)
                    OR b.signer IS NOT DISTINCT FROM ps.authority)
             ORDER BY b.program_id, b.completed_at DESC",
        )
        .bind(&trusted)
        .fetch_all(&self.pool)
        .await?;

        Ok(builds.into_iter().map(Into::into).collect())
    }

    /// Every program ID the sweep should refresh: existing `program_state`
    /// rows, plus completed builds (so a program with a build but no state
    /// row yet -- e.g. after a dropped webhook -- gets bootstrapped).
    /// Ordered oldest-first so a partial cycle still drains the staleness.
    pub async fn sweep_program_ids(&self) -> Result<Vec<String>> {
        Ok(sqlx::query_scalar(
            "SELECT program_id FROM (
                 SELECT ps.program_id, ps.last_checked
                 FROM program_state ps
                 UNION
                 SELECT b.program_id, NULL::timestamptz AS last_checked
                 FROM (SELECT DISTINCT program_id FROM builds WHERE status = 'completed') b
                 WHERE NOT EXISTS (SELECT 1 FROM program_state ps WHERE ps.program_id = b.program_id)
             ) q
             ORDER BY last_checked ASC NULLS FIRST",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// Proxy for "is the sweep still running" -- used by the health endpoints.
    pub async fn last_sweep_at(&self) -> Result<Option<DateTime<Utc>>> {
        Ok(
            sqlx::query_scalar("SELECT MAX(last_checked) FROM program_state")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    /// Records the on-disk log filename for a failed build.
    pub async fn insert_build_log(
        &self,
        build_id: Uuid,
        program_id: &Address,
        file_name: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO build_logs (id, program_id, file_name) VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE SET file_name = EXCLUDED.file_name",
        )
        .bind(build_id)
        .bind(program_id)
        .bind(file_name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Looks up the on-disk log filename for a build.
    pub async fn get_build_log_file(&self, build_id: Uuid) -> Result<Option<String>> {
        Ok(sqlx::query_scalar(
            "SELECT file_name FROM build_logs WHERE id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(build_id)
        .fetch_optional(&self.pool)
        .await?)
    }
}
