-- This file should undo anything in `up.sql`

DROP INDEX IF EXISTS idx_program_address;
DROP TABLE IF EXISTS build_logs;
