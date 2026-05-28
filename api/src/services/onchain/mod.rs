//! On-chain data retrieval services for Solana programs
//!
//! This module provides services for:
//! - Program authority lookup
//! - On-Chain Program hash lookup
//! - Program OtterVerify PDA metadata retrieval

/// Program authority lookup service
pub mod program_authority_retriever;

/// Program hash verification service
pub mod program_hash_retriver;

/// Program metadata retrieval service
pub mod program_metadata_retriever;

pub use program_authority_retriever::{get_program_state, snapshot_programs, ProgramOnchainState};
pub use program_hash_retriver::get_on_chain_hash;
pub use program_metadata_retriever::{
    get_otter_verify_params, is_program_data_missing, trusted_signers, OtterBuildParams,
    OTTER_VERIFY_PROGRAM_ID,
};
