// @generated automatically by Diesel CLI.

diesel::table! {
    solana_program_builds (id) {
        id -> Varchar,
        repository -> Varchar,
        commit_hash -> Nullable<Varchar>,
        program_id -> Varchar,
        lib_name -> Nullable<Varchar>,
        created_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    verified_programs (id) {
        id -> VarChar,
        program_id -> Varchar,
        is_verified -> Bool,
        verified_at -> Nullable<Timestamp>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(solana_program_builds, verified_programs,);
