//! Background directory scanning implementation using the `ignore` crate.

use crate::{
    fs::FileInfo, // Use FileInfo from the same module
    model::FileNode,
    task::ScanMessage,
};
use crossbeam_channel::select;
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
    time::{Duration, Instant}, // Added for batch timeout
}; // Added for select! macro

/// Defines the maximum number of nodes to buffer before sending a batch.
const BATCH_SIZE: usize = 100;
/// Defines the maximum time to wait before sending an incomplete batch.
const BATCH_TIMEOUT: Duration = Duration::from_millis(50);

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
/// The core function executed by the scanner thread.
///
/// It spawns the parallel directory walk in a sub-thread and then processes
/// the results, batching them before sending to the main UI thread.
fn scan_worker(
    root: PathBuf,
    show_hidden: bool,
    ui_sender: Sender<ScanMessage>, // Renamed for clarity
    cancel_signal: Arc<AtomicBool>,
) {
    log::info!("Scan worker started for path: {}", root.display());

    // Create an intermediate channel for walker threads to send results to this worker thread.
    // The type is Result<FileNode, String> to handle both successful nodes and errors.
    let (local_node_tx, local_node_rx) = crossbeam_channel::unbounded::<Result<FileNode, String>>();

    // --- Walker Thread ---
    // Spawn the actual directory walk in a separate thread so the current thread can process results.
    let walker_cancel_signal = cancel_signal.clone();
    let walker_thread = thread::spawn(move || {
        // Configure the parallel directory walker
        let walker = WalkBuilder::new(&root)
            .hidden(!show_hidden)
            .parents(true)
            .ignore(true)
            .git_global(true)
            .git_ignore(true)
            .git_exclude(true)
            .threads(num_cpus::get().min(8))
            .build_parallel();

        // Run the walker, processing results in parallel closures
        walker.run(|| {
            // Clone sender and cancel signal for the closure
            let node_tx = local_node_tx.clone(); // Send to the intermediate channel
            let cancel = walker_cancel_signal.clone();

            // Return a boxed closure that processes each DirEntry result
            Box::new(move |result| {
                // Check for cancellation signal
                if cancel.load(Ordering::Relaxed) {
                    return WalkState::Quit;
                }

                match result {
                    Ok(entry) => {
                        if entry.file_type().is_some_and(|ft| ft.is_symlink()) {
                            log::trace!("Skipping symlink: {}", entry.path().display());
                            return WalkState::Continue;
                        }

                        let path = entry.path();
                        match FileInfo::from_entry(&entry) {
                            Ok(file_info) => {
                                let node = FileNode::new(file_info);
                                // Send Ok(node) to the intermediate channel
                                if node_tx.send(Ok(node)).is_err() {
                                    log::warn!(
                                        "Local node send failed: Channel closed. Quitting walk."
                                    );
                                    return WalkState::Quit;
                                }
                            }
                            Err(e) => {
                                let error_msg =
                                    format!("Failed to process entry '{}': {}", path.display(), e);
                                log::warn!("{}", error_msg);
                                // Send Err(msg) to the intermediate channel
                                if node_tx.send(Err(error_msg)).is_err() {
                                    log::warn!(
                                        "Local error send failed: Channel closed. Quitting walk."
                                    );
                                    return WalkState::Quit;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Filesystem walk error: {}", e);
                        log::error!("{}", error_msg);
                        // Send Err(msg) to the intermediate channel
                        if node_tx.send(Err(error_msg)).is_err() {
                            log::warn!(
                                "Local walk error send failed: Channel closed. Quitting walk."
                            );
                            return WalkState::Quit;
                        }
                    }
                }
                WalkState::Continue
            })
        });
        // Implicitly drop local_node_tx here when the walker thread finishes, closing the channel.
        log::info!("Walker thread finished.");
    });

    // --- Batching and Sending Loop (in scan_worker thread) ---
    let mut node_batch: Vec<FileNode> = Vec::with_capacity(BATCH_SIZE);
    let mut last_batch_send_time = Instant::now();
    // Removed walk_finished variable as it was unused

    loop {
        // Check for cancellation signal periodically
        if cancel_signal.load(Ordering::Relaxed) {
            log::info!("Scan worker detected cancellation signal.");
            // We still need to process remaining messages from the walker thread
            // before fully stopping, so we don't break here immediately.
            // The walker thread itself will quit sending new messages.
        }

        // Use select! to wait for messages or timeout
        select! {
            // Receive results from the intermediate channel
            recv(local_node_rx) -> msg_result => {
                match msg_result {
                    Ok(result) => {
                        match result {
                            Ok(node) => {
                                node_batch.push(node);
                                // Send batch if full
                                // Send batch if full (Combined condition to fix clippy::collapsible_if)
                                // The !is_empty() check is technically redundant if BATCH_SIZE > 0, but kept for clarity.
                                if node_batch.len() >= BATCH_SIZE && !node_batch.is_empty() {
                                    log::trace!("Sending batch of {} nodes (size limit)", node_batch.len());
                                    if ui_sender.send(ScanMessage::AddNodes(std::mem::take(&mut node_batch))).is_err() {
                                        log::warn!("UI sender channel closed while sending full batch.");
                                        break; // Exit loop if UI channel is closed
                                    }
                                    last_batch_send_time = Instant::now(); // Reset timer after sending
                                }
                            }
                            Err(error_msg) => {
                                // Send errors immediately
                                log::warn!("Received error from walker: {}", error_msg);
                                if ui_sender.send(ScanMessage::Error(error_msg)).is_err() {
                                    log::warn!("UI sender channel closed while sending error.");
                                    break; // Exit loop
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Channel closed, meaning the walker thread finished.
                        log::info!("Local node channel closed; walker finished.");
                        // walk_finished = true; // Removed unused assignment
                        break; // Exit the select! loop to send final batch and finish.
                    }
                }
            },
            // Send batch if timeout reached
            default(BATCH_TIMEOUT) => {
                if !node_batch.is_empty() && last_batch_send_time.elapsed() >= BATCH_TIMEOUT {
                    log::trace!("Sending batch of {} nodes (timeout)", node_batch.len());
                    if ui_sender.send(ScanMessage::AddNodes(std::mem::take(&mut node_batch))).is_err() {
                        log::warn!("UI sender channel closed while sending timed-out batch.");
                        break; // Exit loop
                    }
                    last_batch_send_time = Instant::now(); // Reset timer
                }
            }
        }
        // If the walk finished in the recv arm, break the outer loop too
        // Removed check for unused walk_finished variable
        // The loop now breaks directly when the recv returns Err
    }

    // Send any remaining nodes after the loop finishes
    if !node_batch.is_empty() {
        log::info!("Sending final batch of {} nodes.", node_batch.len());
        let _ = ui_sender.send(ScanMessage::AddNodes(node_batch)); // Ignore error if UI closed
    }

    // Wait for the walker thread to complete its cleanup (optional but good practice)
    if let Err(e) = walker_thread.join() {
        log::error!("Walker thread panicked: {:?}", e);
        // Send an error message to UI if walker panicked
        let _ = ui_sender.send(ScanMessage::Error("Walker thread panicked.".to_string()));
    } else {
        log::info!("Walker thread joined successfully.");
    }

    // Log final status based on cancellation signal
    if cancel_signal.load(Ordering::Relaxed) {
        log::info!("Scan worker finished processing due to cancellation signal.");
    } else {
        log::info!("Scan worker finished processing path normally.");
    }

    // Always send the Finished message
    let _ = ui_sender.send(ScanMessage::Finished);
    log::info!("Scan worker thread exiting.");
}
