//! Defines the `FileInfo` struct containing metadata about a file or directory.

use humansize::{format_size, DECIMAL};
use serde::{Deserialize, Serialize};
use std::{
    fs::Metadata, // Keep metadata import for clarity
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Detailed information about a discovered file or directory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// The absolute path to the file or directory.
    pub path: PathBuf,
    /// True if this entry is a directory.
    pub is_dir: bool,
    /// The size of the file in bytes. Directories usually have a system-dependent size (often 0 or 4096).
    pub size: u64,
    /// Human-readable representation of the size (e.g., "1.2 MB").
    #[serde(with = "crate::fs::file_info::humansize_serde")] // Use custom serializer
    pub human_size: String,
    /// Heuristic check if the file content appears to be binary (contains null bytes).
    /// Always false for directories.
    pub is_binary: bool,
    /// The last modified timestamp, if available.
    pub modified: Option<SystemTime>,
    /// The file extension (e.g., "rs", "txt"), lowercased, if present.
    /// None for directories or files without extensions.
    pub extension: Option<String>,
}

impl FileInfo {
    /// Creates a `FileInfo` instance from an `ignore::DirEntry`.
    ///
    /// Attempts to get metadata efficiently from the `DirEntry`. Falls back to
    /// `std::fs::metadata` if the initial attempt fails (e.g., due to permissions).
    /// Performs a binary check for files.
    pub fn from_entry(entry: &ignore::DirEntry) -> anyhow::Result<Self> {
        let path = entry.path().to_path_buf();

        // Try getting metadata from the entry first (often cached by `ignore`)
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                // Fallback to std::fs::metadata if ignore's fails
                // This might happen if permissions change between listing and metadata access.
                log::warn!(
                    "ignore::DirEntry metadata failed for '{}': {}. Falling back to std::fs::metadata.",
                    path.display(),
                    e
                );
                // Propagate the error if the fallback also fails
                std::fs::metadata(&path).map_err(|fs_err| {
                    anyhow::anyhow!(
                        "Failed to get metadata for '{}': ignore error ({}), fs error ({})",
                        path.display(),
                        e,
                        fs_err
                    )
                })?
            }
        };

        Self::from_metadata(path, metadata)
    }

    /// Creates a `FileInfo` instance from a path and `std::fs::Metadata`.
    /// Separated logic for potential reuse or testing.
    pub fn from_metadata(path: PathBuf, metadata: Metadata) -> anyhow::Result<Self> {
        let is_dir = metadata.is_dir();
        let size = metadata.len();
        let human_size = format_size(size, DECIMAL);
        let modified = metadata.modified().ok(); // Convert Result to Option, ignore error

        let mut file_is_binary = false;
        let mut extension = None;

        if !is_dir {
            // Check binary status only for files
            // This involves reading a small part of the file.
            file_is_binary = match is_binary(&path, 8192) {
                // Check first 8KB
                Ok(b) => b,
                Err(e) => {
                    // Log the error but don't fail the whole process; assume not binary.
                    log::warn!(
                        "Failed to perform binary check for '{}': {}. Assuming text.",
                        path.display(),
                        e
                    );
                    false
                }
            };
            // Extract and lowercase the extension
            extension = path
                .extension()
                .and_then(|s| s.to_str()) // Convert OsStr to &str
                .map(|s| s.to_lowercase()); // Convert to lowercase
        }

        Ok(Self {
            path,
            is_dir,
            size,
            human_size,
            is_binary: file_is_binary,
            modified,
            extension,
        })
    }

    /// Returns the final component of the path (file or directory name) as a string slice.
    /// Returns `None` if the path terminates in `..`.
    pub fn file_name(&self) -> Option<&str> {
        self.path.file_name().and_then(|s| s.to_str())
    }
}

/// Checks if a file appears to be binary by reading a sample and looking for null bytes.
///
/// # Arguments
/// * `path` - Path to the file to check.
/// * `sample_size` - Maximum number of bytes to read from the beginning of the file.
///
/// # Returns
/// * `Ok(true)` if a null byte is found within the sample.
/// * `Ok(false)` if no null byte is found or the file is empty/inaccessible.
/// * `Err(anyhow::Error)` for critical I/O errors (excluding NotFound).
pub fn is_binary(path: &Path, sample_size: usize) -> anyhow::Result<bool> {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        // If the file disappeared between listing and checking, treat as not binary.
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        // Propagate other file opening errors.
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to open file '{}' for binary check: {}",
                path.display(),
                e
            ))
        }
    };

    // Use a reasonably sized buffer. Avoid allocating huge buffers.
    let buffer_size = sample_size.min(8192); // Limit buffer to 8KB max
    let mut buffer = vec![0; buffer_size];

    // Read a sample from the file
    let bytes_read = match file.read(&mut buffer) {
        Ok(n) => n,
        // If the file disappeared during read, treat as not binary.
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        // Propagate other read errors.
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to read file '{}' for binary check: {}",
                path.display(),
                e
            ))
        }
    };

    // Check if the read sample contains a null byte (common indicator of binary data)
    Ok(buffer[..bytes_read].contains(&0))
}

/// Serde helper module for serializing/deserializing the `human_size` field.
/// Currently, only serialization is strictly needed as it's derived, but deserialize is included for completeness.
mod humansize_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(size_str: &str, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(size_str)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)
    }
}
