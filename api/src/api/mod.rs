//! API module containing route handlers and initialization logic
//!
//! This module is responsible for:
//! - Defining API routes and handlers
//! - Initializing the API router
//! - Providing API documentation through the index endpoint

/// Route handlers for various API endpoints
pub mod handlers;

/// API documentation and index endpoint
pub mod index;

/// Router initialization and configuration
pub mod init;

// Re-export the router initialization function for easier access
pub use init::initialize_router;
