//! Defines `ScanStats` for collecting statistics during directory scanning.

use crate::fs::FileInfo;
use humansize::{format_size, DECIMAL};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

/// Statistics collected during a directory scan.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ScanStats {
    pub total_files: usize,
    pub total_dirs: usize,
    pub total_size_bytes: u64,
    pub file_types: HashMap<String, usize>,
    pub largest_files: Vec<FileStatInfo>,
    pub errors: Vec<String>,
    #[serde(skip)]
    pub language_stats: tokei::Languages,
}

impl Clone for ScanStats {
    fn clone(&self) -> Self {
        Self {
            total_files: self.total_files,
            total_dirs: self.total_dirs,
            total_size_bytes: self.total_size_bytes,
            file_types: self.file_types.clone(),
            largest_files: self.largest_files.clone(),
            errors: self.errors.clone(),
            language_stats: Default::default(),
        }
    }
}

/// Simplified information about a file, used for tracking largest files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileStatInfo {
    pub path: String,
    pub size: u64,
    pub human_size: String,
}

const MAX_LARGEST_FILES: usize = 10;

impl ScanStats {
    /// Updates the statistics based on a discovered `FileInfo`.
    pub fn add_file(&mut self, info: &FileInfo, root_path: &Path) {
        if info.is_dir {
            self.add_dir();
            return;
        }

        self.total_files += 1;
        self.total_size_bytes += info.size;

        let extension_key = info
            .extension
            .as_deref()
            .map_or_else(|| "(no extension)".to_string(), |ext| format!(".{}", ext));
        *self.file_types.entry(extension_key).or_insert(0) += 1;

        if let Some(loc_stats) = &info.loc_stats {
            let language_type =
                tokei::LanguageType::from_path(&info.path, &tokei::Config::default());
            if let Some(lang_type) = language_type {
                let entry = self.language_stats.entry(lang_type).or_default();

                // MODIFIED: Manually add the stats.
                entry.blanks += loc_stats.blanks;
                entry.code += loc_stats.code;
                entry.comments += loc_stats.comments;
                // Also merge the reports, which contain individual file stats.
                entry.reports.extend(loc_stats.reports.clone());
            }
        }

        if info.size > 0
            && (self.largest_files.len() < MAX_LARGEST_FILES
                || info.size > self.largest_files.last().map_or(0, |f| f.size))
        {
            let relative_path = info
                .path
                .strip_prefix(root_path)
                .unwrap_or(&info.path)
                .display()
                .to_string();
            let stat_info = FileStatInfo {
                path: relative_path,
                size: info.size,
                human_size: info.human_size.clone(),
            };
            let pos = self
                .largest_files
                .partition_point(|f| f.size >= stat_info.size);
            if pos < MAX_LARGEST_FILES {
                self.largest_files.insert(pos, stat_info);
                self.largest_files.truncate(MAX_LARGEST_FILES);
            }
        }
    }

    /// Increments the directory count.
    pub fn add_dir(&mut self) {
        self.total_dirs += 1;
    }

    /// Adds an error message encountered during the scan.
    pub fn add_error(&mut self, error: String) {
        match self.errors.len() {
            len if len < 100 => self.errors.push(error),
            100 => self
                .errors
                .push("... more errors truncated ...".to_string()),
            _ => {}
        }
    }

    /// Merges statistics from another `ScanStats` instance.
    pub fn merge(&mut self, other: ScanStats) {
        self.total_files += other.total_files;
        self.total_dirs += other.total_dirs;
        self.total_size_bytes += other.total_size_bytes;
        self.errors.extend(other.errors);
        if self.errors.len() > 101 {
            self.errors.truncate(101);
            if !self.errors.last().is_some_and(|s| s.contains("truncated")) {
                self.errors
                    .push("... more errors truncated ...".to_string());
            }
        }

        for (ext, count) in other.file_types {
            *self.file_types.entry(ext).or_insert(0) += count;
        }

        for (language_type, language) in other.language_stats {
            self.language_stats.insert(language_type, language);
        }

        self.largest_files.extend(other.largest_files);
        self.largest_files
            .sort_unstable_by(|a, b| b.size.cmp(&a.size));
        self.largest_files
            .dedup_by(|a, b| a.path == b.path && a.size == b.size);
        self.largest_files.truncate(MAX_LARGEST_FILES);
    }

    pub fn finalize(&mut self) {
        log::debug!("Finalizing scan statistics.");
    }

    pub fn total_size_human(&self) -> String {
        format_size(self.total_size_bytes, DECIMAL)
    }
}
