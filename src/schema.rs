// @generated automatically by Diesel CLI.

diesel::table! {
    solana_program_builds (program_id) {
        id -> Varchar,
        repository -> Varchar,
        commit_hash -> Nullable<Varchar>,
        program_id -> Varchar,
        lib_name -> Nullable<Varchar>,
        bpf_flag -> Bool,
        created_at -> Timestamp,
    }
}

diesel::table! {
    verified_programs (id) {
        id -> Varchar,
        program_id -> Varchar,
        is_verified -> Bool,
        on_chain_hash -> Varchar,
        executable_hash -> Varchar,
        verified_at -> Timestamp,
    }
}

diesel::joinable!(verified_programs -> solana_program_builds (program_id));

diesel::allow_tables_to_appear_in_same_query!(
    solana_program_builds,
    verified_programs,
);
