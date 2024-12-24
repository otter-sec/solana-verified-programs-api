//! API request handlers for the verification service.
//! Each module corresponds to a specific API endpoint or related group of endpoints.

// Verification-related handlers
pub mod async_verify; // Asynchronous program verification
pub mod sync_verify; // Synchronous program verification
pub mod unverify;
pub mod verification_status; // Program verification status // Program unverification

// Status and information handlers
pub mod job_status; // Build job status
pub mod logs; // Build logs retrieval
pub mod verified_programs_list; // List of verified programs
pub mod verified_programs_status; // Status of verified programs

// Re-export handlers for easier access
pub(crate) use async_verify::{process_async_verification, process_async_verification_with_signer};
pub(crate) use job_status::get_job_status;
pub(crate) use logs::get_build_logs;
pub(crate) use sync_verify::process_sync_verification;
pub(crate) use unverify::handle_unverify;
pub(crate) use verification_status::{get_verification_status, get_verification_status_all};
pub(crate) use verified_programs_list::{
    get_verified_programs_list, get_verified_programs_list_paginated,
};
pub(crate) use verified_programs_status::get_verified_programs_status;
