// @generated automatically by Diesel CLI.

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