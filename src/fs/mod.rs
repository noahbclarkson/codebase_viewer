//! # Filesystem Module
//!
//! This module contains components related to file system scanning,
//! metadata extraction, and statistics gathering.

pub mod file_info;
pub mod scanner;
pub mod stats;

// Re-export key types for easier access from other modules (like app.rs)
pub use file_info::FileInfo;
pub use scanner::scan; // Re-export the main scan function
pub use stats::ScanStats;
