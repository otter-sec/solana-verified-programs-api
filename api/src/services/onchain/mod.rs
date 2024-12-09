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

// Re-export commonly used functions
pub use program_authority_retriever::get_program_authority;
pub use program_metadata_retriever::{get_otter_verify_params, OtterBuildParams};
