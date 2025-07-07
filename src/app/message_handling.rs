//! Handles messages received from background tasks (scanner, report generator).

use super::state::CodebaseApp;
use crate::external;
use crate::{
    model::FileNode,
    task::{ScanMessage, TaskMessage},
};
use rfd::MessageDialogResult;

impl CodebaseApp {
    /// Central place to handle all background messages.
    pub(super) fn handle_background_messages(&mut self) {
        self.handle_scan_messages();
        self.handle_task_messages();
        self.handle_preview_messages();
    }

    fn handle_scan_messages(&mut self) {
        if let Some(receiver) = self.scan_receiver.clone() {
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    ScanMessage::AddNode(node) => self.add_single_node(node),
                    ScanMessage::AddNodes(nodes) => {
                        for node in nodes {
                            self.add_single_node(node);
                        }
                    }
                    ScanMessage::Error(err_msg) => {
                        log::error!("Scan error reported: {}", err_msg);
                        self.status_message = format!("Scan error: {}", err_msg);
                        if let Some(stats) = self.scan_stats.as_mut() {
                            stats.add_error(err_msg);
                        }
                    }
                    ScanMessage::Progress(msg) => self.status_message = msg,
                    ScanMessage::Stats(partial_stats) => {
                        if let Some(total_stats) = self.scan_stats.as_mut() {
                            total_stats.merge(partial_stats);
                        }
                    }
                    ScanMessage::Finished => {
                        log::info!("Scan finished message received.");
                        self.is_scanning = false;
                        self.background_task = None;
                        self.scan_receiver = None;
                        if let Some(stats) = self.scan_stats.as_mut() {
                            stats.finalize();
                            self.status_message = format!(
                                "Scan complete: {} files, {} dirs, {}",
                                stats.total_files,
                                stats.total_dirs,
                                stats.total_size_human()
                            );
                        } else {
                            self.status_message = "Scan complete.".to_string();
                        }
                        if !self.orphaned_children.is_empty() {
                            log::warn!(
                                "Scan finished with {} unresolved orphan parent path(s).",
                                self.orphaned_children.len()
                            );
                            for (parent, children) in &self.orphaned_children {
                                log::warn!(" - Missing Parent: {}", parent.display());
                                for (_, child_path) in children.iter().take(5) {
                                    log::warn!("   - Orphaned Child: {}", child_path.display());
                                }
                                if children.len() > 5 {
                                    log::warn!(
                                        "   - ...and {} more orphans for this parent",
                                        children.len() - 5
                                    );
                                }
                            }
                            self.orphaned_children.clear();
                        }
                        self.sort_nodes_recursively(self.root_id);
                        if let Some(stats) = &self.scan_stats {
                            if self.config.auto_expand_limit > 0
                                && stats.total_files <= self.config.auto_expand_limit
                            {
                                log::info!(
                                    "Auto-expanding nodes as total file count ({}) <= limit ({}).",
                                    stats.total_files,
                                    self.config.auto_expand_limit
                                );
                                if let Some(root_id) = self.root_id {
                                    if let Some(root_node) = self.nodes.get(root_id) {
                                        let children = root_node.children.clone();
                                        for child_id in children {
                                            if let Some(child_node) = self.nodes.get_mut(child_id) {
                                                child_node.is_expanded = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_task_messages(&mut self) {
        if let Some(receiver) = self.task_receiver.clone() {
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    TaskMessage::ReportProgress(status) => self.status_message = status,
                    TaskMessage::ReportFinished(result) => {
                        self.is_generating_report = false;
                        self.background_task = None;
                        match result {
                            Ok(save_path) => {
                                self.status_message =
                                    format!("Report saved successfully to {}", save_path.display());
                                log::info!(
                                    "Report generated successfully to {}",
                                    save_path.display()
                                );
                                if rfd::MessageDialog::new()
                                    .set_level(rfd::MessageLevel::Info)
                                    .set_title("Report Generated")
                                    .set_description(format!(
                                        "Report saved to:\n{}\n\nWould you like to open it?",
                                        save_path.display()
                                    ))
                                    .set_buttons(rfd::MessageButtons::YesNo)
                                    .show()
                                    == MessageDialogResult::Yes
                                {
                                    if let Err(e) = external::open_path_in_external_app(&save_path)
                                    {
                                        log::error!("Failed to open generated report file: {}", e);
                                        self.status_message =
                                            format!("Error opening report: {}", e);
                                        rfd::MessageDialog::new()
                                            .set_level(rfd::MessageLevel::Error)
                                            .set_title("Open Report Error")
                                            .set_description(format!(
                                                "Could not open report file:\n{}",
                                                e
                                            ))
                                            .show();
                                    }
                                }
                            }
                            Err(err_msg) => {
                                log::error!("Report generation failed: {}", err_msg);
                                self.status_message =
                                    format!("Error generating report: {}", err_msg);
                                rfd::MessageDialog::new()
                                    .set_level(rfd::MessageLevel::Error)
                                    .set_title("Report Generation Failed")
                                    .set_description(&err_msg)
                                    .show();
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_preview_messages(&mut self) {
        if let Some(rx) = &self.preview_receiver {
            for (id, cache_entry) in rx.try_iter() {
                if self.selected_node_id == Some(id) {
                    log::trace!("Received preview for selected node {}", id);
                    self.preview_cache =
                        Some(std::sync::Arc::new(parking_lot::Mutex::new(cache_entry)));
                // MODIFIED
                } else {
                    log::trace!(
                        "Received preview for node {}, but node {:?} is selected. Ignoring.",
                        id,
                        self.selected_node_id
                    );
                }
            }
        }
    }

    /// Helper function to process the addition of a single FileNode.
    fn add_single_node(&mut self, node: FileNode) {
        let node_path = node.path().to_path_buf();
        let node_id = self.nodes.len();
        self.nodes.push(node);
        self.path_to_id_map.insert(node_path.clone(), node_id);
        if self.root_id.is_none() && Some(&node_path) == self.root_path.as_ref() {
            self.root_id = Some(node_id);
            if let Some(root_node) = self.nodes.get_mut(node_id) {
                root_node.is_expanded = true;
            }
            log::debug!(
                "Root node added: ID {}, Path: {}",
                node_id,
                node_path.display()
            );
        }
        let mut parent_found = false;
        if let Some(parent_path) = node_path.parent().map(|p| p.to_path_buf()) {
            if let Some(&parent_id) = self.path_to_id_map.get(&parent_path) {
                if let Some(parent_node) = self.nodes.get_mut(parent_id) {
                    parent_node.children.push(node_id);
                    parent_found = true;
                    if let Some(orphans) = self.orphaned_children.remove(&node_path) {
                        log::debug!(
                            "Node {} ({}) resolved {} orphans.",
                            node_id,
                            node_path.display(),
                            orphans.len()
                        );
                        if let Some(new_parent_node) = self.nodes.get_mut(node_id) {
                            for (orphan_id, _) in orphans {
                                new_parent_node.children.push(orphan_id);
                            }
                        }
                    }
                }
            }
        }
        if !parent_found && self.root_id != Some(node_id) {
            if let Some(parent_path) = node_path.parent() {
                log::trace!(
                    "Orphaned node {} (parent {} not found yet). Storing.",
                    node_path.display(),
                    parent_path.display()
                );
                self.orphaned_children
                    .entry(parent_path.to_path_buf())
                    .or_default()
                    .push((node_id, node_path.clone()));
            } else if self.root_id.is_none() {
                log::error!(
                    "Node {} has no parent but is not root, and root not yet found.",
                    node_path.display()
                );
            }
        }
        if let Some(stats) = self.scan_stats.as_mut() {
            if let Some(root_p) = &self.root_path {
                if let Some(added_node_info) = self.nodes.get(node_id).map(|n| &n.info) {
                    stats.add_file(added_node_info, root_p);
                }
            }
        }
    }
}
