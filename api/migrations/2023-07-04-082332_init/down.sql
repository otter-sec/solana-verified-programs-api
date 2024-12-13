-- This file should undo anything in `up.sql`
-- Drop indexes
DROP INDEX IF EXISTS solana_program_builds_program_id_idx;
DROP INDEX IF EXISTS solana_program_builds_id_idx;
DROP INDEX IF EXISTS verified_programs_program_id_idx;
DROP INDEX IF EXISTS verified_programs_solana_build_id_idx;
DROP INDEX IF EXISTS program_authority_program_id_index;
DROP INDEX IF EXISTS idx_verified_programs_program_id_is_verified;
DROP INDEX IF EXISTS idx_solana_program_builds_created_at;

-- Drop tables
DROP TABLE IF EXISTS build_logs;
DROP TABLE IF EXISTS program_authority;
DROP TABLE IF EXISTS verified_programs;
DROP TABLE IF EXISTS solana_program_builds;
