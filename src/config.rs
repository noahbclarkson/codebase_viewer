//! Handles loading, saving, and managing application configuration.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufReader, BufWriter},
    path::PathBuf,
};

/// Maximum number of recent projects to store in the configuration.
pub const MAX_RECENT_PROJECTS: usize = 10;

/// Structure holding the application's configurable settings.
/// Persisted as JSON in the user's config directory.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(default)] // Ensures new fields get default values if missing from config file
pub struct AppConfig {
    /// UI theme setting: "light", "dark", or "system".
    pub theme: String,
    // pub color_theme: String, // Example: Could be used for accent colors later
    /// Automatically expand directories with fewer than or equal to this many *files* after scan.
    pub auto_expand_limit: usize,
    /// Maximum file size in bytes allowed for previewing or including in reports.
    /// Set to -1 for no limit (use with caution!).
    pub max_file_size_preview: i64,
    /// Whether the scanner should include hidden files and directories (starting with '.').
    pub show_hidden_files: bool,
    /// Default format for generated reports: "markdown", "html", "text".
    pub export_format: String,
    /// Default setting for including scan statistics in reports.
    pub export_include_stats: bool,
    /// Default setting for including selected file contents in reports.
    pub export_include_contents: bool,
    /// List of recently opened project directory paths (absolute paths).
    pub recent_projects: Vec<PathBuf>,
}

impl Default for AppConfig {
    /// Provides default values for the application configuration.
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            // color_theme: "blue".to_string(),
            auto_expand_limit: 100,
            max_file_size_preview: 1_048_576, // 1 MiB
            show_hidden_files: false,
            export_format: "markdown".to_string(),
            export_include_stats: true,
            export_include_contents: true,
            recent_projects: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Loads configuration from the default platform-specific path.
    ///
    /// If the file doesn't exist, is inaccessible, or fails to parse,
    /// it logs an error and returns the default configuration.
    pub fn load() -> Self {
        match config_file() {
            Ok(path) => {
                if path.exists() {
                    log::info!("Attempting to load configuration from {}", path.display());
                    match fs::File::open(&path) {
                        Ok(file) => {
                            let reader = BufReader::new(file);
                            match serde_json::from_reader(reader) {
                                Ok(config) => {
                                    log::info!("Configuration loaded successfully.");
                                    config
                                }
                                Err(e) => {
                                    log::error!(
                                        "Failed to parse config file {}: {}. Using defaults.",
                                        path.display(),
                                        e
                                    );
                                    // Optional: Could attempt to backup the corrupted file here
                                    Self::default()
                                }
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "Failed to open config file {}: {}. Using defaults.",
                                path.display(),
                                e
                            );
                            Self::default()
                        }
                    }
                } else {
                    log::info!(
                        "Config file not found at {}. Using defaults.",
                        path.display()
                    );
                    Self::default() // No config file exists, use defaults
                }
            }
            Err(e) => {
                log::error!("Failed to determine config file path: {e}. Using defaults.");
                Self::default()
            }
        }
    }

    /// Saves the current configuration to the default platform-specific path.
    ///
    /// Creates the configuration directory if it doesn't exist.
    /// Logs errors encountered during saving.
    /// Returns `Ok(())` on success, `Err` on failure.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_file()?;
        log::info!("Attempting to save configuration to {}", path.display());

        // Ensure the parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create config directory {}: {}",
                    parent.display(),
                    e
                )
            })?;
        }

        // Write the config file using pretty JSON
        let file = BufWriter::new(fs::File::create(&path).map_err(|e| {
            anyhow::anyhow!("Failed to create config file {}: {}", path.display(), e)
        })?);
        serde_json::to_writer_pretty(file, self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize or write config: {}", e))?;

        log::info!("Configuration saved successfully.");
        Ok(())
    }

    /// Adds a project path to the beginning of the recent projects list.
    ///
    /// Ensures the path is absolute and canonicalized. Removes duplicates.
    /// Truncates the list if it exceeds `MAX_RECENT_PROJECTS`.
    pub fn add_recent_project(&mut self, path: PathBuf) {
        // Attempt to get the absolute, canonical path for reliable comparison
        let abs_path = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                log::warn!(
                    "Failed to canonicalize path '{}': {}. Using original path if absolute.",
                    path.display(),
                    e
                );
                // Use the original path only if it's already absolute
                if path.is_absolute() {
                    path
                } else {
                    log::error!(
                        "Cannot add relative path to recent projects: {}",
                        path.display()
                    );
                    return; // Don't add relative paths that failed canonicalization
                }
            }
        };

        log::debug!("Adding recent project: {}", abs_path.display());

        // Remove any existing entry for the same path before adding it to the front
        self.recent_projects.retain(|p| {
            // Compare canonicalized paths if possible for robustness
            match p.canonicalize() {
                Ok(canon_p) => canon_p != abs_path,
                Err(_) => p != &abs_path, // Fallback to direct comparison if canonicalization fails for existing entry
            }
        });

        // Insert the new path at the beginning of the list
        self.recent_projects.insert(0, abs_path);

        // Ensure the list doesn't exceed the maximum allowed size
        self.recent_projects.truncate(MAX_RECENT_PROJECTS);
    }

    /// Clears the list of recent projects.
    pub fn clear_recent_projects(&mut self) {
        if !self.recent_projects.is_empty() {
            self.recent_projects.clear();
            log::info!("Recent projects list cleared.");
        }
    }
}

/// Returns the platform-specific configuration directory path for this application.
/// Uses `dirs-next` crate for reliable paths.
/// Example: `~/.config/codebase_viewer` on Linux.
pub fn config_dir() -> anyhow::Result<PathBuf> {
    dirs_next::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine user config directory"))?
        .join("codebase_viewer") // App-specific subdirectory
        .pipe(Ok) // Use std::pipe::Pipe::pipe for cleaner chaining
}

/// Returns the full path to the configuration file (`config.json`).
pub fn config_file() -> anyhow::Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

// Helper trait for cleaner chaining (requires Rust 1.76+)
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}
impl<T> Pipe for T {}
