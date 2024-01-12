-- Jobs table 

ALTER TABLE solana_program_builds ADD COLUMN status VARCHAR(20) NOT NULL DEFAULT 'in_progress';

-- Drop foreign key constraint in the dependent table
ALTER TABLE verified_programs
DROP CONSTRAINT verified_programs_program_id_fkey;

-- Drop the primary key constraint in the solana_program_builds table
ALTER TABLE solana_program_builds
DROP CONSTRAINT solana_program_builds_pkey;

-- Add a new primary key constraint on the program_id column
ALTER TABLE solana_program_builds
ADD PRIMARY KEY (id);
