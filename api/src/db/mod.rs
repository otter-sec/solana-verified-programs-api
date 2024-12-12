//! Database module containing all database-related operations and models.
//! This module provides a structured way to interact with both PostgreSQL and Redis databases.

// Database operation modules
pub mod authority; // Program authority operations
pub mod connection; // Database connection management
pub mod job; // Job status and management
pub mod logs; // Build logs operations
pub mod models; // Database models and types
pub mod params; // Build parameters operations
pub mod programs; // Program verification status
pub mod redis; // Redis cache operations
pub mod verification; // Program verification operations

// Re-export the DbClient for easier access
pub use connection::DbClient;
