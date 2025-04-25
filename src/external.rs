//! Utilities for interacting with external applications.

use std::path::Path;

/// Opens the given path (file or directory) using the default system application.
///
/// Uses the `open` crate for cross-platform compatibility.
///
/// # Arguments
/// * `path` - The file or directory path to open.
///
/// # Returns
/// * `Ok(())` if the path was opened successfully (or the command was dispatched).
/// * `Err(anyhow::Error)` if the path could not be opened.
pub fn open_path_in_external_app(path: &Path) -> anyhow::Result<()> {
    log::info!("Requesting to open path externally: {}", path.display());
    match open::that(path) {
        Ok(_) => {
            log::info!(
                "Successfully dispatched open command for: {}",
                path.display()
            );
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to open path '{}' externally: {}", path.display(), e);
            log::error!("{}", err_msg);
            // Return a user-friendly error message via anyhow
            Err(anyhow::anyhow!(
                "Could not open '{}' with the default application. System error: {}",
                path.display(),
                e
            ))
        }
    }
}
