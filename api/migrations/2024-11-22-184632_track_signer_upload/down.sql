-- This file should undo anything in `up.sql`
ALTER TABLE solana_program_builds DROP COLUMN signer;
ALTER TABLE verified_programs ADD CONSTRAINT verified_programs_program_id_key UNIQUE (program_id);
