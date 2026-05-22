//! Service layer containing core business logic and external integrations.

pub mod background_jobs;
pub mod logging;
pub mod misc;
pub mod onchain;
pub mod verification;

pub use misc::build_repository_url;
