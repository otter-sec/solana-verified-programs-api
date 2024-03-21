// @generated automatically by Diesel CLI.

diesel::table! {
    solana_program_builds (id) {
        id -> Varchar,
        repository -> Varchar,
        commit_hash -> Nullable<Varchar>,
        program_id -> Varchar,
        lib_name -> Nullable<Varchar>,
        base_docker_image -> Nullable<Varchar>,
        mount_path -> Nullable<Varchar>,
        cargo_args -> Nullable<Array<Text>>,
        bpf_flag -> Bool,
        created_at -> Timestamp,
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


diesel::table! {
    mainnet_programs (id) {
        id -> Int4,
        project_name -> Nullable<Varchar>,
        program_address -> Varchar,
        buffer_address -> Varchar,
        github_repo -> Nullable<Varchar>,
        has_security_txt -> Bool,
        is_closed -> Bool,
        is_success -> Bool,
        is_processed -> Bool,
        updated_at -> Timestamp,
        last_deployed_slot -> Nullable<Int8>,
        update_authority -> Nullable<Varchar>,
    }
}

diesel::joinable!(verified_programs -> solana_program_builds (solana_build_id));
diesel::allow_tables_to_appear_in_same_query!(solana_program_builds, verified_programs,);