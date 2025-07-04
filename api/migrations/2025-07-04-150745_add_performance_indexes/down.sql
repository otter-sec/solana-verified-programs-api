-- Drop performance indexes

DROP INDEX IF EXISTS idx_verified_programs_is_verified_verified_at;
DROP INDEX IF EXISTS idx_program_authority_updated_at;
DROP INDEX IF EXISTS idx_solana_program_builds_program_status;
DROP INDEX IF EXISTS idx_solana_program_builds_signer;
DROP INDEX IF EXISTS idx_build_logs_program_created;
DROP INDEX IF EXISTS idx_verified_programs_active;
DROP INDEX IF EXISTS idx_solana_builds_duplicate_check;