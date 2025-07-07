//! Background directory scanning implementation using the `ignore` crate.

use crate::{fs::FileInfo, model::FileNode, task::ScanMessage};
use crossbeam_channel::select;
use crossbeam_channel::Sender;
use ignore::{WalkBuilder, WalkState};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread, // MODIFIED: import thread
    thread::JoinHandle,
    time::{Duration, Instant},
};

const BATCH_SIZE: usize = 100;
const BATCH_TIMEOUT: Duration = Duration::from_millis(50);

pub fn scan(
    root: PathBuf,
    show_hidden: bool,
    sender: Sender<ScanMessage>,
) -> (JoinHandle<()>, Arc<AtomicBool>) {
    let cancel_signal = Arc::new(AtomicBool::new(false));
    let cancel_signal_clone = cancel_signal.clone();
    let root_clone = root.clone();

    log::info!(
        "Spawning scan worker thread for path: '{}', show_hidden: {}",
        root_clone.display(),
        show_hidden
    );

    let handle = thread::Builder::new() // MODIFIED
        .name("scan_worker".to_string())
        .spawn(move || {
            scan_worker(root_clone, show_hidden, sender, cancel_signal_clone);
        })
        .expect("Failed to spawn scan worker thread");

    (handle, cancel_signal)
}

fn scan_worker(
    root: PathBuf,
    show_hidden: bool,
    ui_sender: Sender<ScanMessage>,
    cancel_signal: Arc<AtomicBool>,
) {
    log::info!("Scan worker started for path: {}", root.display());

    let (local_node_tx, local_node_rx) = crossbeam_channel::unbounded::<Result<FileNode, String>>();

    let walker_cancel_signal = cancel_signal.clone();
    let walker_thread = thread::Builder::new() // MODIFIED
        .name("ignore_walker".to_string())
        .spawn(move || {
            let walker = WalkBuilder::new(&root)
                .hidden(!show_hidden)
                .parents(true)
                .ignore(true)
                .git_global(true)
                .git_ignore(true)
                .git_exclude(true)
                .threads(num_cpus::get().min(8))
                .build_parallel();

            walker.run(|| {
                let node_tx = local_node_tx.clone();
                let cancel = walker_cancel_signal.clone();

                Box::new(move |result| {
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
                                    if node_tx.send(Ok(node)).is_err() {
                                        log::warn!("Local node send failed: Channel closed. Quitting walk.");
                                        return WalkState::Quit;
                                    }
                                }
                                Err(e) => {
                                    let error_msg = format!("Failed to process entry '{}': {}", path.display(), e);
                                    log::warn!("{}", error_msg);
                                    if node_tx.send(Err(error_msg)).is_err() {
                                        log::warn!("Local error send failed: Channel closed. Quitting walk.");
                                        return WalkState::Quit;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Filesystem walk error: {}", e);
                            log::error!("{}", error_msg);
                            if node_tx.send(Err(error_msg)).is_err() {
                                log::warn!("Local walk error send failed: Channel closed. Quitting walk.");
                                return WalkState::Quit;
                            }
                        }
                    }
                    WalkState::Continue
                })
            });
            log::info!("Walker thread finished.");
        })
        .expect("Failed to spawn walker thread");

    let mut node_batch: Vec<FileNode> = Vec::with_capacity(BATCH_SIZE);
    let mut last_batch_send_time = Instant::now();

    loop {
        if cancel_signal.load(Ordering::Relaxed) {
            log::info!("Scan worker detected cancellation signal.");
        }

        select! {
            recv(local_node_rx) -> msg_result => {
                match msg_result {
                    Ok(result) => {
                        match result {
                            Ok(node) => {
                                node_batch.push(node);
                                if node_batch.len() >= BATCH_SIZE && !node_batch.is_empty() {
                                    log::trace!("Sending batch of {} nodes (size limit)", node_batch.len());
                                    if ui_sender.send(ScanMessage::AddNodes(std::mem::take(&mut node_batch))).is_err() {
                                        log::warn!("UI sender channel closed while sending full batch.");
                                        break;
                                    }
                                    last_batch_send_time = Instant::now();
                                }
                            }
                            Err(error_msg) => {
                                log::warn!("Received error from walker: {}", error_msg);
                                if ui_sender.send(ScanMessage::Error(error_msg)).is_err() {
                                    log::warn!("UI sender channel closed while sending error.");
                                    break;
                                }
                            }
                        }
                    }
                    Err(_) => {
                        log::info!("Local node channel closed; walker finished.");
                        break;
                    }
                }
            },
            default(BATCH_TIMEOUT) => {
                if !node_batch.is_empty() && last_batch_send_time.elapsed() >= BATCH_TIMEOUT {
                    log::trace!("Sending batch of {} nodes (timeout)", node_batch.len());
                    if ui_sender.send(ScanMessage::AddNodes(std::mem::take(&mut node_batch))).is_err() {
                        log::warn!("UI sender channel closed while sending timed-out batch.");
                        break;
                    }
                    last_batch_send_time = Instant::now();
                }
            }
        }
    }

    if !node_batch.is_empty() {
        log::info!("Sending final batch of {} nodes.", node_batch.len());
        let _ = ui_sender.send(ScanMessage::AddNodes(node_batch));
    }

    if let Err(e) = walker_thread.join() {
        log::error!("Walker thread panicked: {:?}", e);
        let _ = ui_sender.send(ScanMessage::Error("Walker thread panicked.".to_string()));
    } else {
        log::info!("Walker thread joined successfully.");
    }

    if cancel_signal.load(Ordering::Relaxed) {
        log::info!("Scan worker finished processing due to cancellation signal.");
    } else {
        log::info!("Scan worker finished processing path normally.");
    }

    let _ = ui_sender.send(ScanMessage::Finished);
    log::info!("Scan worker thread exiting.");
}
