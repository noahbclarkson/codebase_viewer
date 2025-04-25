//! # Report Generation Module
//!
//! This module defines the structures and functions for generating
//! reports (Markdown, HTML, Text) based on the scanned codebase data
//! and user selections.

use crate::fs::ScanStats; // Use ScanStats from the fs module
use serde::{Deserialize, Serialize};

/// Defines the output format for the generated report.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum ReportFormat {
    /// Markdown format (.md).
    #[default]
    Markdown,
    /// HTML format (.html).
    Html,
    /// Plain text format (.txt).
    Text,
}

/// Options controlling the content and format of the generated report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)] // Added traits for potential saving/comparison
pub struct ReportOptions {
    /// The desired output format.
    pub format: ReportFormat,
    /// Whether to include the overall scan statistics section.
    pub include_stats: bool,
    /// Whether to include the content of selected files.
    pub include_contents: bool,
}

/// Holds details about a single file included in the report content section.
#[derive(Debug, Clone)]
pub struct FileDetail {
    /// Path relative to the scanned root directory.
    pub relative_path: String,
    /// Human-readable file size (e.g., "12.3 kB").
    pub size: String,
    /// Formatted last modified timestamp (e.g., "YYYY-MM-DD HH:MM:SS").
    pub modified: String,
    /// The content of the file, or an error message explaining why it wasn't included.
    /// `Ok(content)` or `Err(reason)`. Reasons include being binary, exceeding size limit, or read errors.
    pub content: Result<String, String>,
}

/// Contains all the necessary data collected from the application state
/// required to generate a report in any supported format.
/// This struct is passed to the formatting functions (markdown, html, text).
#[derive(Debug, Clone)]
pub struct ReportData {
    /// Name of the project (usually the root directory name).
    pub project_name: String,
    /// Timestamp when the report generation started.
    pub timestamp: String,
    /// The absolute root path of the scanned directory.
    pub root_path: String,
    /// A string representing the full directory tree structure.
    pub full_tree_structure: String,
    /// A string representing the tree structure of only the selected items.
    pub selected_tree_structure: String,
    /// Details (including content, if requested) of selected files.
    pub file_details: Vec<FileDetail>,
    /// Overall scan statistics, if requested (`ReportOptions::include_stats`).
    pub stats: Option<ScanStats>,
}

// --- Submodules ---
pub mod generator;
pub mod html;
pub mod markdown;
pub mod text;

// --- Re-exports ---
// Expose the primary functions for use by the main application logic.
pub use generator::{collect_report_data, format_report_content, generate_report};
