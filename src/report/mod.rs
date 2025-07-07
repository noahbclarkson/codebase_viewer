//! # Report Generation Module
//!
//! This module defines the structures and functions for generating
//! reports (Markdown, HTML, Text) based on the scanned codebase data
//! and user selections.

use crate::{config::AppConfig, fs::ScanStats};
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

impl ReportFormat {
    /// Returns the file extension for the format.
    pub fn extension(&self) -> &'static str {
        match self {
            ReportFormat::Markdown => "md",
            ReportFormat::Html => "html",
            ReportFormat::Text => "txt",
        }
    }
}

/// Options controlling the content and format of the generated report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportOptions {
    pub format: ReportFormat,
    pub include_stats: bool,
    pub include_contents: bool,
}

impl ReportOptions {
    /// Creates a `ReportOptions` instance from the defaults in `AppConfig`.
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            format: match config.export_format.as_str() {
                "html" => ReportFormat::Html,
                "text" => ReportFormat::Text,
                _ => ReportFormat::Markdown,
            },
            include_stats: config.export_include_stats,
            include_contents: config.export_include_contents,
        }
    }
}

/// Holds details about a single file included in the report content section.
#[derive(Debug, Clone)]
pub struct FileDetail {
    pub relative_path: String,
    pub size: String,
    pub modified: String,
    pub content: Result<String, String>,
}

/// Contains all the necessary data collected from the application state
/// required to generate a report in any supported format.
#[derive(Debug, Clone)]
pub struct ReportData {
    pub project_name: String,
    pub timestamp: String,
    pub root_path: String,
    pub full_tree_structure: String,
    pub selected_tree_structure: String,
    pub file_details: Vec<FileDetail>,
    pub stats: Option<ScanStats>,
}

// --- Submodules ---
pub mod generator;
pub mod html;
pub mod markdown;
pub mod text;

// --- Re-exports ---
pub use generator::{collect_report_data, format_report_content, generate_report};