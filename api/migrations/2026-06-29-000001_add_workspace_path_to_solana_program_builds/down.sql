-- Remove additional solana-verify build option columns from solana_program_builds table.
ALTER TABLE solana_program_builds DROP COLUMN cargo_build_sbf_args;
ALTER TABLE solana_program_builds DROP COLUMN workspace_path;
