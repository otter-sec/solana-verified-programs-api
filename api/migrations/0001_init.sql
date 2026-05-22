-- v2 schema. Idempotent so this same file works on:
--   1. a fresh database
--   2. a v1 database (Diesel-managed schema with solana_program_builds /
--      verified_programs / program_authority)
-- After this migration runs, v1 tables are gone and v2 carries the data.

DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'build_status') THEN
    CREATE TYPE build_status AS ENUM ('in_progress', 'completed', 'failed');
  END IF;
END $$;

CREATE TABLE IF NOT EXISTS builds (
    id                UUID PRIMARY KEY,
    repository        TEXT NOT NULL,
    commit_hash       TEXT,
    program_id        TEXT NOT NULL,
    lib_name          TEXT,
    base_docker_image TEXT,
    mount_path        TEXT,
    cargo_args        TEXT[],
    bpf_flag          BOOLEAN NOT NULL DEFAULT FALSE,
    arch              TEXT,
    signer            TEXT,
    status            build_status NOT NULL,
    executable_hash   TEXT,
    error_message     TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at      TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS builds_executable_hash_idx ON builds (executable_hash) WHERE status = 'completed';
CREATE INDEX IF NOT EXISTS builds_program_id_created_idx ON builds (program_id, created_at DESC);
CREATE INDEX IF NOT EXISTS builds_program_completed_idx ON builds (program_id, completed_at DESC) WHERE status = 'completed';

CREATE TABLE IF NOT EXISTS program_state (
    program_id    TEXT PRIMARY KEY,
    on_chain_hash TEXT,
    authority     TEXT,
    is_frozen     BOOLEAN NOT NULL DEFAULT FALSE,
    is_closed     BOOLEAN NOT NULL DEFAULT FALSE,
    last_checked  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS program_state_last_checked_idx ON program_state (last_checked ASC);

-- v1 build_logs (program_address VARCHAR, id VARCHAR, created_at TIMESTAMP)
-- needs to be reshaped before the v2 CREATE TABLE IF NOT EXISTS below
-- becomes a no-op.
DO $$ BEGIN
  IF EXISTS (
      SELECT 1 FROM information_schema.columns
      WHERE table_name = 'build_logs' AND column_name = 'program_address'
  ) THEN
    ALTER TABLE build_logs RENAME COLUMN program_address TO program_id;
    ALTER TABLE build_logs ALTER COLUMN id TYPE UUID USING id::uuid;
    ALTER TABLE build_logs ALTER COLUMN created_at TYPE TIMESTAMPTZ USING created_at AT TIME ZONE 'UTC';
    -- v1's index was on the old column name; drop and let the new one take over.
    DROP INDEX IF EXISTS idx_build_logs_program_created;
  END IF;
END $$;

CREATE TABLE IF NOT EXISTS build_logs (
    id           UUID PRIMARY KEY,
    program_id   TEXT NOT NULL,
    file_name    TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS build_logs_program_idx ON build_logs (program_id, created_at DESC);

-- v1 -> v2 data move. Wrapped in a single DO block so it's atomic with the
-- DROP TABLEs at the end. If anything inside fails postgres rolls everything
-- back (sqlx wraps the migration in a transaction too).
DO $$ BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'solana_program_builds') THEN
    -- builds <- solana_program_builds JOIN verified_programs
    INSERT INTO builds (
      id, repository, commit_hash, program_id, lib_name, base_docker_image,
      mount_path, cargo_args, bpf_flag, arch, signer, status, executable_hash,
      error_message, created_at, completed_at
    )
    SELECT
      spb.id::uuid,
      spb.repository,
      spb.commit_hash,
      spb.program_id,
      spb.lib_name,
      spb.base_docker_image,
      spb.mount_path,
      spb.cargo_args,
      COALESCE(spb.bpf_flag, false),
      spb.arch,
      spb.signer,
      CASE spb.status
        WHEN 'completed'   THEN 'completed'::build_status
        WHEN 'in_progress' THEN 'in_progress'::build_status
        ELSE 'failed'::build_status  -- maps both 'failed' and the historical 'un-used'
      END,
      vp.executable_hash,
      NULL,
      spb.created_at AT TIME ZONE 'UTC',
      CASE WHEN spb.status = 'completed' THEN vp.verified_at AT TIME ZONE 'UTC' END
    FROM solana_program_builds spb
    LEFT JOIN verified_programs vp ON vp.solana_build_id = spb.id
    ON CONFLICT (id) DO NOTHING;

    -- program_state <- latest verified_programs row per program + program_authority
    INSERT INTO program_state (program_id, on_chain_hash, authority, is_frozen, is_closed, last_checked)
    SELECT
      COALESCE(latest_vp.program_id, pa.program_id),
      latest_vp.on_chain_hash,
      pa.authority_id,
      COALESCE(pa.is_frozen, false),
      COALESCE(pa.is_closed, false),
      NOW()
    FROM (
      SELECT DISTINCT ON (program_id) program_id, on_chain_hash, verified_at
      FROM verified_programs
      ORDER BY program_id, verified_at DESC
    ) latest_vp
    FULL OUTER JOIN program_authority pa ON pa.program_id = latest_vp.program_id
    ON CONFLICT (program_id) DO NOTHING;

    -- Order matters: verified_programs has a FK to solana_program_builds.
    DROP TABLE verified_programs;
    DROP TABLE program_authority;
    DROP TABLE solana_program_builds;
  END IF;
END $$;
