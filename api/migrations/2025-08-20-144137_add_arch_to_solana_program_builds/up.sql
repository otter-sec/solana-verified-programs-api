-- Add arch column to solana_program_builds table
ALTER TABLE solana_program_builds ADD COLUMN arch VARCHAR(3) DEFAULT NULL;
