//! Background directory scanning implementation using the `ignore` crate.

use crate::{
    fs::FileInfo, // Use FileInfo from the same module
    model::FileNode,
    task::ScanMessage,
};
use crossbeam_channel::Sender;
use ignore::{WalkBuilder, WalkState};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    thread::JoinHandle,
};

/// Spawns a background thread to scan the specified directory.
///
/// Uses `ignore::WalkBuilder` for parallel, gitignore-aware directory traversal.
/// Sends `ScanMessage` updates back to the UI thread via the provided `sender`.
///
/// # Arguments
/// * `root` - The root directory path to start scanning from.
/// * `show_hidden` - Whether to include hidden files/directories.
/// * `sender` - The channel sender to send `ScanMessage` updates to.
///
/// # Returns
/// A tuple containing:
/// * `JoinHandle<()>` - Handle for the spawned scanner thread.
/// * `Arc<AtomicBool>` - Signal used to request cancellation of the scan.
pub fn scan(
    root: PathBuf,
    show_hidden: bool,
    sender: Sender<ScanMessage>,
) -> (JoinHandle<()>, Arc<AtomicBool>) {
    // Create an atomic boolean flag for cancellation signalling.
    let cancel_signal = Arc::new(AtomicBool::new(false));
    let cancel_signal_clone = cancel_signal.clone(); // Clone for the thread
    let root_clone = root.clone(); // Clone root path for the thread

    log::info!(
        "Spawning scan worker thread for path: '{}', show_hidden: {}",
        root_clone.display(),
        show_hidden
    );

    // Spawn the worker thread
    let handle = thread::spawn(move || {
        scan_worker(root_clone, show_hidden, sender, cancel_signal_clone);
    });

    (handle, cancel_signal)
}

/// The core function executed by the scanner thread.
///
/// Configures and runs the `ignore::WalkParallel` iterator, processing
/// each directory entry and sending messages back to the UI thread.
fn scan_worker(
    root: PathBuf,
    show_hidden: bool,
    sender: Sender<ScanMessage>,
    cancel_signal: Arc<AtomicBool>,
) {
    log::info!("Scan worker started for path: {}", root.display());

    // Configure the parallel directory walker
    let walker = WalkBuilder::new(&root)
        .hidden(!show_hidden) // Respect hidden file setting
        .parents(true) // Include parent directories (needed for structure)
        .ignore(true) // Enable reading .ignore files
        .git_global(true) // Respect global gitignore
        .git_ignore(true) // Respect .gitignore in repository
        .git_exclude(true) // Respect .git/info/exclude
        .threads(num_cpus::get().min(8)) // Use multiple threads (up to 8 or num CPUs)
        .build_parallel(); // Build the parallel walker

    // Run the walker, processing results in parallel closures
    walker.run(|| {
        // Clone sender and cancel signal for the closure
        let sender = sender.clone();
        let cancel = cancel_signal.clone();

        // Return a boxed closure that processes each DirEntry result
        Box::new(move |result| {
            // Check for cancellation signal before processing each entry
            if cancel.load(Ordering::Relaxed) {
                return WalkState::Quit; // Stop walking if cancelled
            }

            match result {
                Ok(entry) => {
                    // Skip entries that are symbolic links to avoid cycles and redundant processing
                    // Note: `ignore` crate might handle some symlink cases, but explicit check is safer.
                    if entry.file_type().is_some_and(|ft| ft.is_symlink()) {
                        log::trace!("Skipping symlink: {}", entry.path().display());
                        return WalkState::Continue;
                    }

                    let path = entry.path();
                    // Attempt to create FileInfo from the entry
                    match FileInfo::from_entry(&entry) {
                        Ok(file_info) => {
                            // Create a FileNode from the FileInfo
                            let node = FileNode::new(file_info);
                            // Send the new node back to the UI thread
                            if sender.send(ScanMessage::AddNode(node)).is_err() {
                                // If sending fails, the UI thread likely terminated; stop scanning.
                                log::warn!(
                                    "Scan AddNode send failed: Channel closed. Quitting walk."
                                );
                                return WalkState::Quit;
                            }
                        }
                        Err(e) => {
                            // Failed to process metadata or binary check for an entry
                            let error_msg =
                                format!("Failed to process entry '{}': {}", path.display(), e);
                            log::warn!("{}", error_msg);
                            // Send the error message back to the UI thread
                            if sender.send(ScanMessage::Error(error_msg)).is_err() {
                                log::warn!(
                                    "Scan Error send failed: Channel closed. Quitting walk."
                                );
                                return WalkState::Quit;
                            }
                            // Continue walking even if one entry fails
                        }
                    }
                }
                Err(e) => {
                    // An error occurred during the directory traversal itself
                    let error_msg = format!("Filesystem walk error: {}", e);
                    log::error!("{}", error_msg);
                    // Send the error message back to the UI thread
                    if sender.send(ScanMessage::Error(error_msg)).is_err() {
                        log::warn!("Scan Error send failed: Channel closed. Quitting walk.");
                        return WalkState::Quit;
                    }
                    // Continue walking if possible after a traversal error
                }
            }
            WalkState::Continue // Continue to the next entry
        })
    });

    // Log whether the scan finished normally or was cancelled
    if cancel_signal.load(Ordering::Relaxed) {
        log::info!("Scan worker finished processing due to cancellation signal.");
    } else {
        log::info!("Scan worker finished processing path: {}", root.display());
    }

    // Always send the Finished message, regardless of cancellation status.
    // The UI thread uses this to know the task is complete.
    // Use `let _ = ...` to ignore potential send error if channel is already closed.
    let _ = sender.send(ScanMessage::Finished);
    log::info!("Scan worker thread exiting for path: {}", root.display());
}
