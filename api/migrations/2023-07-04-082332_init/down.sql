-- This file should undo anything in `up.sql`
drop index solana_program_builds_program_id_idx;
drop index verified_programs_program_id_idx;
drop table verified_programs;
drop table solana_program_builds;