CREATE TABLE IF NOT EXISTS solana_program_builds (
    id VARCHAR(36) NOT NULL,
    repository VARCHAR NOT NULL,
    commit_hash VARCHAR,
    program_id VARCHAR(44) NOT NULL,
    lib_name VARCHAR,
    base_docker_image VARCHAR,
    mount_path VARCHAR,
    cargo_args TEXT[],
    bpf_flag BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    status VARCHAR(20) NOT NULL DEFAULT 'in_progress' 
        CHECK (status IN ('in_progress', 'completed', 'failed', 'un-used')),
    signer VARCHAR,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS verified_programs (
    id VARCHAR(36) PRIMARY KEY,
    program_id VARCHAR(44) NOT NULL,
    is_verified BOOLEAN NOT NULL,
    on_chain_hash VARCHAR NOT NULL,
    executable_hash VARCHAR NOT NULL,
    verified_at TIMESTAMP NOT NULL DEFAULT NOW(),
    solana_build_id VARCHAR(36) NOT NULL,
    FOREIGN KEY (solana_build_id) REFERENCES solana_program_builds (id)
);

CREATE TABLE IF NOT EXISTS program_authority (
    program_id VARCHAR(44) NOT NULL,
    authority_id VARCHAR(44),
    last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (program_id)
);

CREATE TABLE IF NOT EXISTS build_logs (
    id VARCHAR(36) NOT NULL,
    program_address VARCHAR(44) NOT NULL,
    file_name VARCHAR NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id)
);

-- Indexes
CREATE INDEX IF NOT EXISTS solana_program_builds_program_id_idx ON solana_program_builds (program_id);
CREATE INDEX IF NOT EXISTS solana_program_builds_id_idx ON solana_program_builds (id);
CREATE INDEX IF NOT EXISTS verified_programs_program_id_idx ON verified_programs (program_id);
CREATE INDEX IF NOT EXISTS verified_programs_solana_build_id_idx ON verified_programs (solana_build_id);
CREATE INDEX IF NOT EXISTS program_authority_program_id_index ON program_authority (program_id);
CREATE INDEX IF NOT EXISTS idx_verified_programs_program_id_is_verified ON verified_programs(program_id, is_verified);
CREATE INDEX IF NOT EXISTS idx_solana_program_builds_created_at ON solana_program_builds(created_at DESC);
