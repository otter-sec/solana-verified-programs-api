// @generated automatically by Diesel CLI.

diesel::table! {
    build_logs (id) {
        id -> Uuid,
        program_address -> Varchar,
        file_name -> Varchar,
        created_at -> Timestamp,
    }
}

diesel::table! {
    solana_program_builds (id) {
        id -> Varchar,
        repository -> Varchar,
        commit_hash -> Nullable<Varchar>,
        program_id -> Varchar,
        lib_name -> Nullable<Varchar>,
        base_docker_image -> Nullable<Varchar>,
        mount_path -> Nullable<Varchar>,
        cargo_args -> Nullable<Array<Nullable<Text>>>,
        bpf_flag -> Bool,
        created_at -> Timestamp,
        #[max_length = 20]
        status -> Varchar,
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
        solana_build_id -> Varchar,
    }
}

diesel::joinable!(verified_programs -> solana_program_builds (solana_build_id));

diesel::allow_tables_to_appear_in_same_query!(
    build_logs,
    solana_program_builds,
    verified_programs,
);
