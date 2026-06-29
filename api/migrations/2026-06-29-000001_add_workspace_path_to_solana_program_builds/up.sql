-- Preserve additional solana-verify build options for monorepos and toolchain-specific builds.
ALTER TABLE solana_program_builds ADD COLUMN workspace_path VARCHAR DEFAULT NULL;
ALTER TABLE solana_program_builds ADD COLUMN cargo_build_sbf_args VARCHAR DEFAULT NULL;
