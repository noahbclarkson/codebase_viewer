//! Defines the `FileInfo` struct containing metadata about a file or directory.

use humansize::{format_size, DECIMAL};
use serde::{Deserialize, Serialize};
use std::{
    fs::Metadata,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Detailed information about a discovered file or directory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    #[serde(with = "crate::fs::file_info::humansize_serde")]
    pub human_size: String,
    pub is_binary: bool,
    pub modified: Option<SystemTime>,
    pub extension: Option<String>,
    #[serde(skip)]
    pub loc_stats: Option<tokei::Language>,
}

impl FileInfo {
    /// Creates a `FileInfo` instance from an `ignore::DirEntry`.
    pub fn from_entry(entry: &ignore::DirEntry) -> anyhow::Result<Self> {
        let path = entry.path().to_path_buf();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                log::warn!("ignore::DirEntry metadata failed for '{}': {}. Falling back to std::fs::metadata.", path.display(), e);
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
    pub fn from_metadata(path: PathBuf, metadata: Metadata) -> anyhow::Result<Self> {
        let is_dir = metadata.is_dir();
        let size = metadata.len();
        let human_size = format_size(size, DECIMAL);
        let modified = metadata.modified().ok();

        let mut file_is_binary = false;
        let mut extension = None;
        let mut loc_stats = None;

        if !is_dir {
            file_is_binary = match is_binary(&path, 8192) {
                Ok(b) => b,
                Err(e) => {
                    log::warn!(
                        "Failed to perform binary check for '{}': {}. Assuming text.",
                        path.display(),
                        e
                    );
                    false
                }
            };
            extension = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase());

            if !file_is_binary {
                loc_stats = get_loc_stats(&path);
            }
        }

        Ok(Self {
            path,
            is_dir,
            size,
            human_size,
            is_binary: file_is_binary,
            modified,
            extension,
            loc_stats,
        })
    }

    /// Returns the final component of the path (file or directory name) as a string slice.
    pub fn file_name(&self) -> Option<&str> {
        self.path.file_name().and_then(|s| s.to_str())
    }
}

/// Checks if a file appears to be binary by reading a sample and looking for null bytes.
pub fn is_binary(path: &Path, sample_size: usize) -> anyhow::Result<bool> {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to open file '{}' for binary check: {}",
                path.display(),
                e
            ))
        }
    };
    let buffer_size = sample_size.min(8192);
    let mut buffer = vec![0; buffer_size];
    let bytes_read = match file.read(&mut buffer) {
        Ok(n) => n,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to read file '{}' for binary check: {}",
                path.display(),
                e
            ))
        }
    };
    Ok(buffer[..bytes_read].contains(&0))
}

/// Get LOC stats for a single file, returning the `Language` struct.
fn get_loc_stats(path: &Path) -> Option<tokei::Language> {
    let mut languages = tokei::Languages::new();
    let config = tokei::Config::default();
    languages.get_statistics(&[path], &[], &config);

    // MODIFIED: `from_path` returns an Option, which we must handle.
    let language_type = tokei::LanguageType::from_path(path, &config)?;
    languages.remove(&language_type)
}

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
