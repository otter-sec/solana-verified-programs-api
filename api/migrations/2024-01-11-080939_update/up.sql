-- Jobs column 

ALTER TABLE solana_program_builds ADD COLUMN status VARCHAR(20) NOT NULL DEFAULT 'in_progress';

-- For 1:1 mapping between solana_program_builds and verified_programs
ALTER TABLE verified_programs ADD COLUMN solana_build_id VARCHAR NOT NULL DEFAULT 'null';

-- Update the solana_build_id column in verified_programs based on the corresponding id from solana_program_builds
UPDATE verified_programs SET solana_build_id = solana_program_builds.id
FROM solana_program_builds WHERE verified_programs.program_id = solana_program_builds.program_id;


-- Add a foreign key constraint on the solana_build_id column
ALTER TABLE solana_program_builds ADD CONSTRAINT solana_program_builds_id_unique UNIQUE (id);
ALTER TABLE verified_programs ADD CONSTRAINT verified_programs_solana_build_id_fkey FOREIGN KEY (solana_build_id) REFERENCES solana_program_builds (id);

-- Drop foreign key constraint in the dependent table
ALTER TABLE verified_programs
DROP CONSTRAINT verified_programs_program_id_fkey;

-- Drop the primary key constraint in the solana_program_builds table
ALTER TABLE solana_program_builds
DROP CONSTRAINT solana_program_builds_pkey;

-- Add a new primary key constraint on the program_id column
ALTER TABLE solana_program_builds ADD PRIMARY KEY (id);

-- Create index on solana_program_builds table for id column
CREATE INDEX IF NOT EXISTS solana_program_builds_id_idx ON solana_program_builds (id);

-- Create index on verified_programs table for solana_build_id column
CREATE INDEX IF NOT EXISTS verified_programs_solana_build_id_idx ON verified_programs (solana_build_id);
