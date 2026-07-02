//! Library entry point. The binary in `src/main.rs` just wires these
//! modules together; tests under `tests/` exercise them directly.

pub mod api;
pub mod build;
pub mod config;
pub mod db;
pub mod errors;
pub mod onchain;
pub mod state;
pub mod sweep;
pub mod types;

/// Result type for the API.
pub type Result<T> = std::result::Result<T, errors::ApiError>;
