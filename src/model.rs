//! Core data model definitions for the application.

use crate::fs::FileInfo; // Use FileInfo from the fs module
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Represents the tri-state selection status of a node in the file tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Check {
    /// The node and all its descendants (if any) are deselected.
    Unchecked,
    /// The node and all its descendants (if any) are selected. This is the default.
    #[default]
    Checked,
    /// For directories, indicates that some descendants are selected and some are not.
    /// This state is calculated based on children's states.
    Partial,
}

/// Represents a node in the file tree, which can be a file or a directory.
/// Nodes are stored in a `Vec<FileNode>` arena in `CodebaseApp`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    /// Metadata about the file or directory (path, size, type, etc.).
    pub info: FileInfo,
    /// Indices (`FileId`) of the direct children of this node in the `nodes` arena.
    /// Empty for files.
    pub children: Vec<FileId>,
    /// The current selection state of this node.
    #[serde(default)] // Ensures deserialization works if state is missing
    pub state: Check,
    /// Whether the node (if a directory) is currently expanded in the tree view.
    #[serde(default)]
    pub is_expanded: bool,
}

/// Type alias for the index into the `CodebaseApp::nodes` vector, uniquely identifying a node.
pub type FileId = usize;

impl FileNode {
    /// Creates a new `FileNode` from `FileInfo`.
    /// Initializes with default state (Checked, Collapsed).
    pub fn new(info: FileInfo) -> Self {
        Self {
            info,
            children: Vec::new(),
            state: Check::default(), // Default is Checked
            is_expanded: false,      // Default is collapsed
        }
    }

    /// Returns `true` if the node represents a directory.
    #[inline]
    pub fn is_dir(&self) -> bool {
        self.info.is_dir
    }

    /// Returns a reference to the full path of the file/directory.
    #[inline]
    pub fn path(&self) -> &Path {
        &self.info.path
    }

    /// Returns the display name (file or directory name) of the node.
    /// Falls back to the full path string if the name cannot be extracted.
    pub fn name(&self) -> &str {
        self.info
            .file_name()
            .unwrap_or_else(|| self.path().to_str().unwrap_or("[Invalid Path]"))
    }
}
