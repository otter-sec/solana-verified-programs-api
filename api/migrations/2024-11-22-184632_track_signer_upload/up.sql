-- Your SQL goes here
ALTER TABLE solana_program_builds ADD COLUMN signer VARCHAR;
ALTER TABLE verified_programs DROP CONSTRAINT verified_programs_program_id_key; 