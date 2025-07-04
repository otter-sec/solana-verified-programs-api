-- Add performance indexes for optimized queries

-- Index for efficient verification lookups by program_id and verification status
CREATE INDEX IF NOT EXISTS idx_verified_programs_is_verified_verified_at ON verified_programs(is_verified, verified_at DESC) WHERE is_verified = true;

-- Index for efficient program authority lookups
CREATE INDEX IF NOT EXISTS idx_program_authority_updated_at ON program_authority(last_updated DESC);

-- Composite index for solana builds by program_id and status
CREATE INDEX IF NOT EXISTS idx_solana_program_builds_program_status ON solana_program_builds(program_id, status, created_at DESC);

-- Index for efficient signer lookups
CREATE INDEX IF NOT EXISTS idx_solana_program_builds_signer ON solana_program_builds(signer) WHERE signer IS NOT NULL;

-- Index for efficient build log lookups
CREATE INDEX IF NOT EXISTS idx_build_logs_program_created ON build_logs(program_address, created_at DESC);

-- Partial index for active (completed) verified programs
CREATE INDEX IF NOT EXISTS idx_verified_programs_active ON verified_programs(program_id, verified_at DESC) WHERE is_verified = true;

-- Index for efficient duplicate check queries
CREATE INDEX IF NOT EXISTS idx_solana_builds_duplicate_check ON solana_program_builds(program_id, repository, commit_hash, signer) WHERE status != 'failed';