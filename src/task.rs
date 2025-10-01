//! Defines message types used for communication between the UI thread and background tasks.

use crate::{
    fs::ScanStats,                 // Use ScanStats from fs module
    llm::gemini_service::AppError, // Error type for AI queries
    model::FileNode,               // Use FileNode from model module
};
use std::path::PathBuf;

/// Messages sent from the background scanner thread (`fs::scanner`) to the UI thread (`app.rs`).
#[derive(Debug)]
pub enum ScanMessage {
    /// A new file or directory node has been discovered.
    AddNode(FileNode), // Keep for potential single-node updates if needed later
    /// A batch of new file or directory nodes has been discovered.
    AddNodes(Vec<FileNode>),
    /// An error occurred during scanning (e.g., permission denied).
    Error(String),
    /// A progress update message (e.g., "Scanning file X...").
    Progress(String),
    /// Partial scan statistics (optional, if scanner aggregates incrementally).
    Stats(ScanStats),
    /// Indicates that the scan process has finished (normally or due to cancellation/error).
    Finished,
}

/// Messages sent from generic background tasks (currently report generation) to the UI thread.
#[derive(Debug)]
pub enum TaskMessage {
    /// A progress update message from the task (e.g., "Formatting report...").
    ReportProgress(String),
    /// Indicates that the report generation task has finished.
    /// `Ok` contains the path where the report was saved successfully.
    /// `Err` contains an error message describing the failure.
    ReportFinished(Result<PathBuf, String>),
    AIResponse(Result<String, AppError>),
    // Could add other task types here later, e.g., PreviewFinished(Result<PreviewCache, String>)
}
