//! On-chain data retrieval services.
//!
//! - `state`: program authority, on-chain hash, and `program_state` snapshots.
//! - `otter`: Otter Verify PDA params + program-data-account presence.

pub mod otter;
pub mod state;

pub use otter::{
    get_otter_verify_params, is_program_data_missing, trusted_signers, OtterBuildParams,
    OTTER_VERIFY_PROGRAM_ID,
};
pub use state::{get_on_chain_hash, get_program_state, snapshot_programs, ProgramOnchainState};
