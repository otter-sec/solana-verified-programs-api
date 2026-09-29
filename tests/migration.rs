//! v1 (Diesel-era) -> v2 schema migration: seed the old shape, run
//! `db.migrate()`, assert the data ended up in v2's tables and the v1
//! tables are retained (dropped later by a follow-up, not by 0001).

mod common;

use common::pg_for_test;
use verified_programs_api::db::DbClient;

/// Boots a fresh postgres with the v1 (Diesel-era) schema pre-seeded,
/// then runs the v2 migrations on top and confirms the data ended up
/// in the new tables with the right shape.
#[tokio::test]
async fn migrates_v1_schema_to_v2() {
    let (url, _pg) = pg_for_test().await;
    let pool = sqlx::PgPool::connect(&url).await.expect("pool");

    sqlx::raw_sql(
        r#"
        CREATE TABLE solana_program_builds (
            id VARCHAR(36) PRIMARY KEY,
            repository VARCHAR NOT NULL,
            commit_hash VARCHAR,
            program_id VARCHAR(44) NOT NULL,
            lib_name VARCHAR,
            base_docker_image VARCHAR,
            mount_path VARCHAR,
            cargo_args TEXT[],
            bpf_flag BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMP NOT NULL DEFAULT NOW(),
            status VARCHAR(20) NOT NULL,
            signer VARCHAR,
            arch VARCHAR(3)
        );
        CREATE TABLE verified_programs (
            id VARCHAR(36) PRIMARY KEY,
            program_id VARCHAR(44) NOT NULL,
            is_verified BOOLEAN NOT NULL,
            on_chain_hash VARCHAR NOT NULL,
            executable_hash VARCHAR NOT NULL,
            verified_at TIMESTAMP NOT NULL DEFAULT NOW(),
            solana_build_id VARCHAR(36) NOT NULL REFERENCES solana_program_builds(id)
        );
        CREATE TABLE program_authority (
            program_id VARCHAR(44) PRIMARY KEY,
            authority_id VARCHAR(44),
            last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
            is_frozen BOOLEAN DEFAULT FALSE,
            is_closed BOOLEAN NOT NULL DEFAULT FALSE
        );
        CREATE TABLE build_logs (
            id VARCHAR(36) PRIMARY KEY,
            program_address VARCHAR(44) NOT NULL,
            file_name VARCHAR NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT NOW()
        );

        INSERT INTO solana_program_builds VALUES
          ('11111111-1111-1111-1111-111111111111', 'https://github.com/a/b', NULL,
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', NULL, NULL, NULL, NULL,
           true, NOW(), 'completed', '9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU', NULL);
        INSERT INTO verified_programs VALUES
          ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC',
           true, 'hash_v', 'hash_v', NOW(),
           '11111111-1111-1111-1111-111111111111');
        INSERT INTO program_authority VALUES
          ('verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC',
           '9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU',
           NOW(), false, false);
        INSERT INTO build_logs VALUES
          ('44444444-4444-4444-4444-444444444444',
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', 'log_abc', NOW());
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed v1");

    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("db connect");
    db.migrate().await.expect("migrate v1 -> v2");

    let builds: Vec<(uuid::Uuid, String, Option<String>)> =
        sqlx::query_as("SELECT id, program_id, executable_hash FROM builds ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("builds");
    assert_eq!(builds.len(), 1);
    assert_eq!(builds[0].1, "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC");
    assert_eq!(builds[0].2.as_deref(), Some("hash_v"));

    type StateRow = (String, Option<String>, Option<String>, bool, bool);
    let state: Vec<StateRow> = sqlx::query_as(
        "SELECT program_id, on_chain_hash, authority, is_frozen, is_closed
         FROM program_state",
    )
    .fetch_all(&pool)
    .await
    .expect("state");
    assert_eq!(state.len(), 1);
    assert_eq!(state[0].1.as_deref(), Some("hash_v"));
    assert_eq!(
        state[0].2.as_deref(),
        Some("9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU")
    );

    let logs_row: (String, String) = sqlx::query_as("SELECT program_id, file_name FROM build_logs")
        .fetch_one(&pool)
        .await
        .expect("build_logs");
    assert_eq!(logs_row.1, "log_abc");

    let v1_exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS (
             SELECT 1 FROM information_schema.tables
             WHERE table_name = 'solana_program_builds'
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("v1 check");
    assert!(
        v1_exists.0,
        "v1 tables are retained for a reversible cutover (dropped by a follow-up)"
    );
}

/// Running `migrate()` twice on a clean DB should be a no-op the second
/// time -- the `IF NOT EXISTS` / `IF EXISTS` guards in `0001_init.sql`
/// must not error or duplicate data.
#[tokio::test]
async fn migration_idempotent_on_v2_schema() {
    let (url, _pg) = pg_for_test().await;
    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("connect");

    db.migrate().await.expect("first migrate");
    db.migrate().await.expect("second migrate (must be no-op)");

    let pool = sqlx::PgPool::connect(&url).await.expect("pool");
    let v2_tables: (bool,) = sqlx::query_as(
        "SELECT
           EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'builds')
           AND EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'program_state')
           AND EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'build_logs')",
    )
    .fetch_one(&pool)
    .await
    .expect("v2 check");
    assert!(v2_tables.0, "all v2 tables should exist after re-migration");
}

/// Migrate against a v1 schema with no data: tables exist but every
/// row count is zero. The data-move `DO` block runs but inserts
/// nothing; v1 tables are retained either way.
#[tokio::test]
async fn migration_handles_v1_with_no_data() {
    let (url, _pg) = pg_for_test().await;
    let pool = sqlx::PgPool::connect(&url).await.expect("pool");

    sqlx::raw_sql(
        r#"
        CREATE TABLE solana_program_builds (
            id VARCHAR(36) PRIMARY KEY,
            repository VARCHAR NOT NULL,
            commit_hash VARCHAR,
            program_id VARCHAR(44) NOT NULL,
            lib_name VARCHAR,
            base_docker_image VARCHAR,
            mount_path VARCHAR,
            cargo_args TEXT[],
            bpf_flag BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMP NOT NULL DEFAULT NOW(),
            status VARCHAR(20) NOT NULL,
            signer VARCHAR,
            arch VARCHAR(3)
        );
        CREATE TABLE verified_programs (
            id VARCHAR(36) PRIMARY KEY,
            program_id VARCHAR(44) NOT NULL,
            is_verified BOOLEAN NOT NULL,
            on_chain_hash VARCHAR NOT NULL,
            executable_hash VARCHAR NOT NULL,
            verified_at TIMESTAMP NOT NULL DEFAULT NOW(),
            solana_build_id VARCHAR(36) NOT NULL REFERENCES solana_program_builds(id)
        );
        CREATE TABLE program_authority (
            program_id VARCHAR(44) PRIMARY KEY,
            authority_id VARCHAR(44),
            last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
            is_frozen BOOLEAN DEFAULT FALSE,
            is_closed BOOLEAN NOT NULL DEFAULT FALSE
        );
        CREATE TABLE build_logs (
            id VARCHAR(36) PRIMARY KEY,
            program_address VARCHAR(44) NOT NULL,
            file_name VARCHAR NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT NOW()
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed empty v1");

    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("connect");
    db.migrate().await.expect("migrate empty v1");

    for (table, sql) in [
        ("builds", "SELECT COUNT(*) FROM builds"),
        ("program_state", "SELECT COUNT(*) FROM program_state"),
        ("build_logs", "SELECT COUNT(*) FROM build_logs"),
    ] {
        let count: (i64,) = sqlx::query_as(sql).fetch_one(&pool).await.expect("count");
        assert_eq!(count.0, 0, "{table} should be empty");
    }

    let v1_exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS (
             SELECT 1 FROM information_schema.tables
             WHERE table_name = 'solana_program_builds'
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("v1 check");
    assert!(
        v1_exists.0,
        "v1 tables are retained even when empty (dropped by a follow-up)"
    );
}

/// Multiple completed builds for the same program (different commits)
/// must all migrate into `builds`; the `program_state` row uses the
/// latest `verified_at` from `verified_programs`.
#[tokio::test]
async fn migration_preserves_multiple_builds_per_program() {
    let (url, _pg) = pg_for_test().await;
    let pool = sqlx::PgPool::connect(&url).await.expect("pool");

    sqlx::raw_sql(
        r#"
        CREATE TABLE solana_program_builds (
            id VARCHAR(36) PRIMARY KEY,
            repository VARCHAR NOT NULL,
            commit_hash VARCHAR,
            program_id VARCHAR(44) NOT NULL,
            lib_name VARCHAR,
            base_docker_image VARCHAR,
            mount_path VARCHAR,
            cargo_args TEXT[],
            bpf_flag BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMP NOT NULL DEFAULT NOW(),
            status VARCHAR(20) NOT NULL,
            signer VARCHAR,
            arch VARCHAR(3)
        );
        CREATE TABLE verified_programs (
            id VARCHAR(36) PRIMARY KEY,
            program_id VARCHAR(44) NOT NULL,
            is_verified BOOLEAN NOT NULL,
            on_chain_hash VARCHAR NOT NULL,
            executable_hash VARCHAR NOT NULL,
            verified_at TIMESTAMP NOT NULL DEFAULT NOW(),
            solana_build_id VARCHAR(36) NOT NULL REFERENCES solana_program_builds(id)
        );
        CREATE TABLE program_authority (
            program_id VARCHAR(44) PRIMARY KEY,
            authority_id VARCHAR(44),
            last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
            is_frozen BOOLEAN DEFAULT FALSE,
            is_closed BOOLEAN NOT NULL DEFAULT FALSE
        );
        CREATE TABLE build_logs (
            id VARCHAR(36) PRIMARY KEY,
            program_address VARCHAR(44) NOT NULL,
            file_name VARCHAR NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT NOW()
        );

        INSERT INTO solana_program_builds VALUES
          ('11111111-1111-1111-1111-111111111111', 'https://github.com/a/b', 'commit_old',
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', NULL, NULL, NULL, NULL,
           true, '2024-01-01 00:00:00', 'completed', NULL, NULL),
          ('22222222-2222-2222-2222-222222222222', 'https://github.com/a/b', 'commit_mid',
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', NULL, NULL, NULL, NULL,
           true, '2024-06-01 00:00:00', 'completed', NULL, NULL),
          ('33333333-3333-3333-3333-333333333333', 'https://github.com/a/b', 'commit_new',
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', NULL, NULL, NULL, NULL,
           true, '2024-12-01 00:00:00', 'completed', NULL, NULL);
        INSERT INTO verified_programs VALUES
          ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaa01',
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC',
           true, 'hash_new', 'hash_old', '2024-01-02 00:00:00',
           '11111111-1111-1111-1111-111111111111'),
          ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaa02',
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC',
           true, 'hash_new', 'hash_mid', '2024-06-02 00:00:00',
           '22222222-2222-2222-2222-222222222222'),
          ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaa03',
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC',
           true, 'hash_new', 'hash_new', '2024-12-02 00:00:00',
           '33333333-3333-3333-3333-333333333333');
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed multi");

    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("connect");
    db.migrate().await.expect("migrate multi");

    let builds: Vec<(String,)> =
        sqlx::query_as("SELECT commit_hash FROM builds ORDER BY commit_hash")
            .fetch_all(&pool)
            .await
            .expect("builds");
    assert_eq!(builds.len(), 3, "all three builds should migrate");
    assert_eq!(
        builds.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
        vec!["commit_mid", "commit_new", "commit_old"]
    );

    // program_state takes the latest verified_at -> hash_new
    let state: (String, Option<String>) =
        sqlx::query_as("SELECT program_id, on_chain_hash FROM program_state")
            .fetch_one(&pool)
            .await
            .expect("state");
    assert_eq!(state.1.as_deref(), Some("hash_new"));
}

/// `program_authority.authority_id = NULL` (frozen / immutable program)
/// must migrate to `program_state.authority = NULL` -- not the literal
/// string `"NULL"`.
#[tokio::test]
async fn migration_handles_null_authority() {
    let (url, _pg) = pg_for_test().await;
    let pool = sqlx::PgPool::connect(&url).await.expect("pool");

    sqlx::raw_sql(
        r#"
        CREATE TABLE solana_program_builds (
            id VARCHAR(36) PRIMARY KEY, repository VARCHAR NOT NULL, commit_hash VARCHAR,
            program_id VARCHAR(44) NOT NULL, lib_name VARCHAR, base_docker_image VARCHAR,
            mount_path VARCHAR, cargo_args TEXT[], bpf_flag BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMP NOT NULL DEFAULT NOW(), status VARCHAR(20) NOT NULL,
            signer VARCHAR, arch VARCHAR(3)
        );
        CREATE TABLE verified_programs (
            id VARCHAR(36) PRIMARY KEY, program_id VARCHAR(44) NOT NULL,
            is_verified BOOLEAN NOT NULL, on_chain_hash VARCHAR NOT NULL,
            executable_hash VARCHAR NOT NULL, verified_at TIMESTAMP NOT NULL DEFAULT NOW(),
            solana_build_id VARCHAR(36) NOT NULL REFERENCES solana_program_builds(id)
        );
        CREATE TABLE program_authority (
            program_id VARCHAR(44) PRIMARY KEY, authority_id VARCHAR(44),
            last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
            is_frozen BOOLEAN DEFAULT FALSE, is_closed BOOLEAN NOT NULL DEFAULT FALSE
        );
        CREATE TABLE build_logs (
            id VARCHAR(36) PRIMARY KEY, program_address VARCHAR(44) NOT NULL,
            file_name VARCHAR NOT NULL, created_at TIMESTAMP NOT NULL DEFAULT NOW()
        );

        INSERT INTO program_authority VALUES
          ('verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', NULL, NOW(), true, false);
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed null authority");

    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("connect");
    db.migrate().await.expect("migrate");

    let state: (String, Option<String>, bool, bool) =
        sqlx::query_as("SELECT program_id, authority, is_frozen, is_closed FROM program_state")
            .fetch_one(&pool)
            .await
            .expect("state");
    assert_eq!(state.0, "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC");
    assert!(state.1.is_none(), "authority must be NULL, not 'NULL'");
    assert!(state.2, "is_frozen preserved");
}

/// v1 had a historical `'un-used'` status that maps to v2's `failed`
/// alongside `'failed'` itself. Both should land as `failed` in v2.
#[tokio::test]
async fn migration_maps_unused_status_to_failed() {
    let (url, _pg) = pg_for_test().await;
    let pool = sqlx::PgPool::connect(&url).await.expect("pool");

    sqlx::raw_sql(
        r#"
        CREATE TABLE solana_program_builds (
            id VARCHAR(36) PRIMARY KEY, repository VARCHAR NOT NULL, commit_hash VARCHAR,
            program_id VARCHAR(44) NOT NULL, lib_name VARCHAR, base_docker_image VARCHAR,
            mount_path VARCHAR, cargo_args TEXT[], bpf_flag BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMP NOT NULL DEFAULT NOW(), status VARCHAR(20) NOT NULL,
            signer VARCHAR, arch VARCHAR(3)
        );
        CREATE TABLE verified_programs (
            id VARCHAR(36) PRIMARY KEY, program_id VARCHAR(44) NOT NULL,
            is_verified BOOLEAN NOT NULL, on_chain_hash VARCHAR NOT NULL,
            executable_hash VARCHAR NOT NULL, verified_at TIMESTAMP NOT NULL DEFAULT NOW(),
            solana_build_id VARCHAR(36) NOT NULL REFERENCES solana_program_builds(id)
        );
        CREATE TABLE program_authority (
            program_id VARCHAR(44) PRIMARY KEY, authority_id VARCHAR(44),
            last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
            is_frozen BOOLEAN DEFAULT FALSE, is_closed BOOLEAN NOT NULL DEFAULT FALSE
        );
        CREATE TABLE build_logs (
            id VARCHAR(36) PRIMARY KEY, program_address VARCHAR(44) NOT NULL,
            file_name VARCHAR NOT NULL, created_at TIMESTAMP NOT NULL DEFAULT NOW()
        );

        INSERT INTO solana_program_builds VALUES
          ('11111111-1111-1111-1111-111111111111', 'https://github.com/a/b', NULL,
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', NULL, NULL, NULL, NULL,
           true, NOW(), 'un-used', NULL, NULL),
          ('22222222-2222-2222-2222-222222222222', 'https://github.com/a/b', NULL,
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', NULL, NULL, NULL, NULL,
           true, NOW(), 'failed', NULL, NULL);
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed mixed statuses");

    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("connect");
    db.migrate().await.expect("migrate");

    let statuses: Vec<(String,)> = sqlx::query_as("SELECT status::text FROM builds ORDER BY id")
        .fetch_all(&pool)
        .await
        .expect("statuses");
    assert_eq!(statuses.len(), 2);
    assert!(
        statuses.iter().all(|s| s.0 == "failed"),
        "un-used and failed both map to 'failed', got: {statuses:?}"
    );
}

/// A `solana_program_builds` row with no matching `verified_programs`
/// entry (e.g. a build still in-progress at migration time) should
/// migrate with `executable_hash = NULL` and `completed_at = NULL`.
#[tokio::test]
async fn migration_handles_build_without_verified_programs_row() {
    let (url, _pg) = pg_for_test().await;
    let pool = sqlx::PgPool::connect(&url).await.expect("pool");

    sqlx::raw_sql(
        r#"
        CREATE TABLE solana_program_builds (
            id VARCHAR(36) PRIMARY KEY, repository VARCHAR NOT NULL, commit_hash VARCHAR,
            program_id VARCHAR(44) NOT NULL, lib_name VARCHAR, base_docker_image VARCHAR,
            mount_path VARCHAR, cargo_args TEXT[], bpf_flag BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMP NOT NULL DEFAULT NOW(), status VARCHAR(20) NOT NULL,
            signer VARCHAR, arch VARCHAR(3)
        );
        CREATE TABLE verified_programs (
            id VARCHAR(36) PRIMARY KEY, program_id VARCHAR(44) NOT NULL,
            is_verified BOOLEAN NOT NULL, on_chain_hash VARCHAR NOT NULL,
            executable_hash VARCHAR NOT NULL, verified_at TIMESTAMP NOT NULL DEFAULT NOW(),
            solana_build_id VARCHAR(36) NOT NULL REFERENCES solana_program_builds(id)
        );
        CREATE TABLE program_authority (
            program_id VARCHAR(44) PRIMARY KEY, authority_id VARCHAR(44),
            last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
            is_frozen BOOLEAN DEFAULT FALSE, is_closed BOOLEAN NOT NULL DEFAULT FALSE
        );
        CREATE TABLE build_logs (
            id VARCHAR(36) PRIMARY KEY, program_address VARCHAR(44) NOT NULL,
            file_name VARCHAR NOT NULL, created_at TIMESTAMP NOT NULL DEFAULT NOW()
        );

        INSERT INTO solana_program_builds VALUES
          ('11111111-1111-1111-1111-111111111111', 'https://github.com/a/b', NULL,
           'verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC', NULL, NULL, NULL, NULL,
           true, NOW(), 'in_progress', NULL, NULL);
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed inprogress");

    let db = DbClient::connect(&url, 5, std::time::Duration::from_secs(300))
        .await
        .expect("connect");
    db.migrate().await.expect("migrate");

    let row: (
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
        String,
    ) = sqlx::query_as("SELECT executable_hash, completed_at, status::text FROM builds")
        .fetch_one(&pool)
        .await
        .expect("row");
    assert!(row.0.is_none(), "executable_hash must be NULL");
    assert!(row.1.is_none(), "completed_at must be NULL");
    assert_eq!(row.2, "in_progress");
}
