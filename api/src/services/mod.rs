//! Service layer containing core business logic and external integrations
//!
//! This module contains:
//! - Background job management for periodic tasks
//! - Logging services for build outputs
//! - Miscellaneous utility functions
//! - On-chain data retrieval services
//! - Program verification logic
//! - RPC client management with key rotation

/// Background job management for periodic tasks
pub mod background_jobs;

/// Build log management services
pub mod logging;

/// Utility functions and helpers
pub mod misc;

/// On-chain data retrieval services
pub mod onchain;

/// RPC client management with key rotation
pub mod rpc_manager;

/// Program verification services
pub mod verification;

// Re-export commonly used functions
pub use misc::build_repository_url;
pub use onchain::program_hash_retriver::get_on_chain_hash;
