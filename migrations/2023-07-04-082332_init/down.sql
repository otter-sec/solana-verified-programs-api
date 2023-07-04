-- This file should undo anything in `up.sql`
drop table solana_program_builds;
drop table verified_programs;
drop index verified_programs_program_id_idx;