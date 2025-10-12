//! Implements the logic for all `AppAction` variants.

use super::{state::CodebaseApp, AppAction};
use crate::{
    external,
    fs::scanner,
    llm::gemini_service,
    model::Check,
    report::{self, ReportFormat, ReportOptions},
    selection,
    task::TaskMessage,
};
use arboard::Clipboard;
use std::{env, fs as std_fs, path::PathBuf, sync::atomic::Ordering, thread};
use tokio::runtime::Builder;

impl CodebaseApp {
    /// Processes all actions queued in `deferred_actions`.
    pub(super) fn process_deferred_actions(&mut self) {
        let actions_to_process: Vec<AppAction> = self.deferred_actions.drain(..).collect();
        for action in actions_to_process {
            log::debug!("Processing action: {action:?}");
            match action {
                AppAction::ToggleCheckState(id) => self.perform_toggle_check_state(id),
                AppAction::ToggleExpandState(id) => self.perform_toggle_expand_state(id),
                AppAction::SelectAllNodes => self.perform_select_all_nodes(),
                AppAction::DeselectAllNodes => self.perform_deselect_all_nodes(),
                AppAction::ExpandAllNodes => self.perform_expand_all_nodes(),
                AppAction::CollapseAllNodes => self.perform_collapse_all_nodes(),
                AppAction::SelectAllChildren(id) => self.perform_select_all_children(id),
                AppAction::DeselectAllChildren(id) => self.perform_deselect_all_children(id),
                AppAction::OpenNodeExternally(id) => self.perform_open_node_externally(id),
                AppAction::SaveSelection => self.perform_save_selection(),
                AppAction::LoadSelection => self.perform_load_selection(),
                AppAction::GenerateReport(opts) => self.perform_generate_report(opts),
                AppAction::CopyReport(opts) => self.perform_copy_report(opts),
                AppAction::StartScan(path) => self.perform_start_scan(path),
                AppAction::CancelScan => self.perform_cancel_scan(),
                AppAction::FocusSearchBox => self.perform_focus_search_box(),
                AppAction::QueryAI(query) => self.perform_query_ai(query),
            }
        }
    }

    // --- Action Implementations ---

    fn perform_start_scan(&mut self, path: PathBuf) {
        if self.is_scanning || self.is_generating_report {
            log::warn!("Cannot start scan: Another background task is running.");
            self.status_message = "Busy with another task (scan/report).".to_string();
            return;
        }
        log::info!("Starting scan for directory: {}", path.display());
        self.root_path = Some(path.clone());
        self.config.add_recent_project(path.clone());
        if let Err(e) = self.config.save() {
            log::error!("Failed to save config after adding recent project: {e}");
        }
        self.nodes.clear();
        self.root_id = None;
        self.selected_node_id = None;
        self.scan_stats = Some(Default::default());
        self.preview_cache = None;
        self.orphaned_children.clear();
        self.path_to_id_map.clear();
        self.status_message = format!(
            "Scanning {}...",
            path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().to_string()
            )
        );
        self.is_scanning = true;
        let (sender, receiver) = crossbeam_channel::unbounded();
        self.scan_receiver = Some(receiver);
        let (handle, cancel_signal) = scanner::scan(
            path,
            self.config.show_hidden_files,
            self.config.respect_cbvignore,
            sender,
        );
        self.background_task = Some(super::state::BackgroundTask::Scan(handle, cancel_signal));
    }

    fn perform_cancel_scan(&mut self) {
        if let Some(super::state::BackgroundTask::Scan(_, cancel_signal)) = &self.background_task {
            log::info!("Requesting scan cancellation...");
            cancel_signal.store(true, Ordering::Relaxed);
            self.status_message = "Scan cancellation requested...".to_string();
        } else {
            log::warn!("No active scan to cancel.");
        }
    }

    fn perform_toggle_check_state(&mut self, node_id: crate::model::FileId) {
        let new_state = if let Some(node) = self.nodes.get(node_id) {
            match node.state {
                Check::Checked | Check::Partial => Check::Unchecked,
                Check::Unchecked => Check::Checked,
            }
        } else {
            log::warn!("Attempted to toggle check state for invalid node ID: {node_id}");
            return;
        };
        self.set_node_state_recursive(node_id, new_state);
        self.update_parent_states(node_id);
        log::trace!("Toggled check state for node {node_id}");
    }

    fn perform_toggle_expand_state(&mut self, node_id: crate::model::FileId) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            if node.is_dir() {
                node.is_expanded = !node.is_expanded;
                log::trace!("Toggled expand state for node {node_id}");
            }
        } else {
            log::warn!("Attempted to toggle expand state for invalid node ID: {node_id}");
        }
    }

    fn perform_select_all_nodes(&mut self) {
        if let Some(root_id) = self.root_id {
            self.set_node_state_recursive(root_id, Check::Checked);
            log::info!("Selected all nodes.");
        }
    }

    fn perform_deselect_all_nodes(&mut self) {
        if let Some(root_id) = self.root_id {
            self.set_node_state_recursive(root_id, Check::Unchecked);
            log::info!("Deselected all nodes.");
        }
    }

    fn perform_expand_all_nodes(&mut self) {
        if let Some(root_id) = self.root_id {
            self.set_node_expansion_recursive(root_id, true);
            log::info!("Expanded all nodes.");
        }
    }

    fn perform_collapse_all_nodes(&mut self) {
        if let Some(root_id) = self.root_id {
            self.set_node_expansion_recursive(root_id, false);
            if let Some(node) = self.nodes.get_mut(root_id) {
                node.is_expanded = true;
            }
            log::info!("Collapsed all nodes (except root).");
        }
    }

    fn perform_select_all_children(&mut self, node_id: crate::model::FileId) {
        if let Some(node) = self.nodes.get(node_id) {
            if !node.is_dir() {
                return;
            }
            let children = node.children.clone();
            for child_id in children {
                self.set_node_state_recursive(child_id, Check::Checked);
            }
            self.update_parent_states(node_id);
            log::debug!("Selected all children of node {node_id}");
        }
    }

    fn perform_deselect_all_children(&mut self, node_id: crate::model::FileId) {
        if let Some(node) = self.nodes.get(node_id) {
            if !node.is_dir() {
                return;
            }
            let children = node.children.clone();
            for child_id in children {
                self.set_node_state_recursive(child_id, Check::Unchecked);
            }
            self.update_parent_states(node_id);
            log::debug!("Deselected all children of node {node_id}");
        }
    }

    fn perform_open_node_externally(&mut self, node_id: crate::model::FileId) {
        if let Some(node) = self.nodes.get(node_id) {
            let path = node.path();
            log::info!("Attempting to open path externally: {}", path.display());
            if let Err(e) = external::open_path_in_external_app(path) {
                log::error!("Failed to open path externally: {e}");
                self.status_message = format!("Error opening path: {e}");
                rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Error)
                    .set_title("Open Error")
                    .set_description(format!("Could not open '{}':\n{}", path.display(), e))
                    .show();
            }
        } else {
            log::warn!("Attempted to open invalid node ID externally: {node_id}");
        }
    }

    fn perform_save_selection(&mut self) {
        if self.root_path.is_none() || self.root_id.is_none() {
            self.status_message = "No directory open to save selection from.".to_string();
            log::warn!("Save selection attempted with no directory open.");
            return;
        }
        let default_name = format!(
            "{}_selection.json",
            self.root_path
                .as_ref()
                .unwrap()
                .file_name()
                .map_or("codebase", |n| n.to_str().unwrap_or("codebase"))
        );
        if let Some(save_path) = rfd::FileDialog::new()
            .add_filter("JSON Files", &["json"])
            .set_file_name(&default_name)
            .save_file()
        {
            match selection::save_selection_to_file(
                &self.nodes,
                self.root_id,
                self.root_path.as_ref().unwrap(),
                &save_path,
            ) {
                Ok(_) => {
                    self.status_message = format!("Selection saved to {}", save_path.display())
                }
                Err(e) => {
                    log::error!("Failed to save selection: {e}");
                    self.status_message = format!("Error saving selection: {e}");
                    rfd::MessageDialog::new()
                        .set_level(rfd::MessageLevel::Error)
                        .set_title("Save Selection Failed")
                        .set_description(format!("Could not save selection:\n{e}"))
                        .show();
                }
            }
        } else {
            self.status_message = "Save selection cancelled.".to_string();
        }
    }

    fn perform_load_selection(&mut self) {
        if self.root_path.is_none() || self.root_id.is_none() {
            self.status_message = "No directory open to load selection into.".to_string();
            log::warn!("Load selection attempted with no directory open.");
            return;
        }
        if let Some(load_path) = rfd::FileDialog::new()
            .add_filter("JSON Files", &["json"])
            .pick_file()
        {
            match selection::load_selection_from_file(&mut self.nodes, self.root_id, &load_path) {
                Ok(saved_root_path_str) => {
                    let current_root_str = self.root_path.as_ref().unwrap().display().to_string();
                    if saved_root_path_str != current_root_str {
                        log::warn!(
                            "Loaded selection for different root ('{saved_root_path_str}') than current ('{current_root_str}')."
                        );
                        self.status_message = format!(
                            "Warning: Loaded selection for different root: {saved_root_path_str}"
                        );
                        rfd::MessageDialog::new().set_level(rfd::MessageLevel::Warning).set_title("Load Selection Warning").set_description(format!("The loaded selection file was created for a different directory:\n\n{saved_root_path_str}\n\nSelection has been applied based on matching relative paths.")).show();
                    } else {
                        self.status_message =
                            format!("Selection loaded from {}", load_path.display());
                    }
                    if let Some(root_id) = self.root_id {
                        self.recalculate_all_parent_states(root_id);
                    }
                }
                Err(e) => {
                    log::error!("Failed to load selection: {e}");
                    self.status_message = format!("Error loading selection: {e}");
                    rfd::MessageDialog::new()
                        .set_level(rfd::MessageLevel::Error)
                        .set_title("Load Selection Failed")
                        .set_description(format!("Could not load selection:\n{e}"))
                        .show();
                }
            }
        } else {
            self.status_message = "Load selection cancelled.".to_string();
        }
    }

    fn perform_generate_report(&mut self, options: ReportOptions) {
        if self.is_scanning || self.is_generating_report {
            log::warn!("Cannot generate report: Another background task is running.");
            self.status_message = "Busy with another task (scan/report).".to_string();
            return;
        }
        if self.root_path.is_none() || self.root_id.is_none() {
            self.status_message = "No directory open to generate report from.".to_string();
            log::warn!("Generate report attempted with no directory open.");
            return;
        }
        let default_ext = options.format.extension();
        let default_name = format!(
            "{}_report.{}",
            self.root_path
                .as_ref()
                .unwrap()
                .file_name()
                .map_or("codebase", |n| n.to_str().unwrap_or("codebase")),
            default_ext
        );
        if let Some(save_path) = rfd::FileDialog::new()
            .add_filter(
                format!("{:?} Report", options.format).as_str(),
                &[default_ext],
            )
            .set_file_name(&default_name)
            .save_file()
        {
            self.status_message = "Generating report in background...".to_string();
            self.is_generating_report = true;
            let report_data_result = report::collect_report_data(self, &options);
            let task_sender = self.task_sender.clone().expect("Task sender should exist");
            let report_options = options.clone();
            let handle = thread::Builder::new()
                .name("report_generator".to_string())
                .spawn(move || match report_data_result {
                    Ok(data) => {
                        let _ = task_sender.send(TaskMessage::ReportProgress(
                            "Formatting report...".to_string(),
                        ));
                        match report::format_report_content(&data, &report_options) {
                            Ok(report_content) => {
                                let _ = task_sender.send(TaskMessage::ReportProgress(format!(
                                    "Saving report to {}...",
                                    save_path.display()
                                )));
                                match std_fs::write(&save_path, report_content) {
                                    Ok(_) => {
                                        let _ = task_sender
                                            .send(TaskMessage::ReportFinished(Ok(save_path)));
                                    }
                                    Err(e) => {
                                        let err_msg = format!("Failed to write report file: {e}");
                                        log::error!("{err_msg}");
                                        let _ = task_sender
                                            .send(TaskMessage::ReportFinished(Err(err_msg)));
                                    }
                                }
                            }
                            Err(e) => {
                                let err_msg = format!("Failed to format report content: {e}");
                                log::error!("{err_msg}");
                                let _ = task_sender.send(TaskMessage::ReportFinished(Err(err_msg)));
                            }
                        }
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to collect report data: {e}");
                        log::error!("{err_msg}");
                        let _ = task_sender.send(TaskMessage::ReportFinished(Err(err_msg)));
                    }
                })
                .expect("Failed to spawn report generator thread");
            self.background_task = Some(super::state::BackgroundTask::Report(handle));
        } else {
            self.status_message = "Report generation cancelled.".to_string();
        }
    }

    fn perform_copy_report(&mut self, options: ReportOptions) {
        if self.is_scanning || self.is_generating_report {
            log::warn!("Cannot copy report: Another background task is running.");
            self.status_message = "Busy with another task (scan/report).".to_string();
            return;
        }
        if self.root_path.is_none() || self.root_id.is_none() {
            self.status_message = "No directory open to generate report from.".to_string();
            log::warn!("Copy report attempted with no directory open.");
            return;
        }
        match report::generate_report(self, &options) {
            Ok(content) => match Clipboard::new() {
                Ok(mut clipboard) => {
                    if let Err(e) = clipboard.set_text(content) {
                        log::error!("Failed to copy report to clipboard: {e}");
                        self.status_message = format!("Error copying report: {e}");
                        rfd::MessageDialog::new()
                            .set_level(rfd::MessageLevel::Error)
                            .set_title("Copy Report Failed")
                            .set_description(format!("Could not copy report:\n{e}"))
                            .show();
                    } else {
                        self.status_message = "Report copied to clipboard.".to_string();
                    }
                }
                Err(e) => {
                    log::error!("Failed to access clipboard: {e}");
                    self.status_message = format!("Clipboard error: {e}");
                    rfd::MessageDialog::new()
                        .set_level(rfd::MessageLevel::Error)
                        .set_title("Clipboard Error")
                        .set_description(format!("Could not access clipboard:\n{e}"))
                        .show();
                }
            },
            Err(e) => {
                log::error!("Failed to generate report for clipboard: {e}");
                self.status_message = format!("Error generating report: {e}");
                rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Error)
                    .set_title("Report Generation Failed")
                    .set_description(format!("Could not generate report:\n{e}"))
                    .show();
            }
        }
    }

    fn perform_focus_search_box(&mut self) {
        self.focus_search_box = true;
    }

    fn perform_query_ai(&mut self, query: String) {
        if self.is_querying_ai {
            log::warn!("AI query already in progress; ignoring new request.");
            return;
        }

        if self.root_path.is_none() {
            self.status_message = "Open a directory before querying Gemini.".to_string();
            return;
        }

        let trimmed = query.trim();
        if trimmed.is_empty() {
            self.status_message = "Enter a question before querying Gemini.".to_string();
            return;
        }

        let api_key = env::var("GEMINI_API_KEY")
            .ok()
            .or_else(|| self.config.gemini_api_key.clone());

        let Some(api_key) = api_key else {
            let message = "Gemini API key not set. Provide GEMINI_API_KEY env var or configure it in Preferences.".to_string();
            self.ai_response_text = Some(message.clone());
            self.status_message = message;
            log::error!("{}", self.status_message);
            return;
        };

        let Some(task_sender) = self.task_sender.clone() else {
            self.status_message = "Internal error: AI task channel unavailable.".to_string();
            log::error!("{}", self.status_message);
            return;
        };

        self.is_querying_ai = true;
        self.ai_response_text = None;
        self.status_message = "Querying Gemini...".to_string();
        self.show_ai_query_window = true;

        let prompt = trimmed.to_owned();
        let report_options = ReportOptions {
            format: ReportFormat::Markdown,
            include_stats: true,
            include_contents: true,
            include_line_numbers: true,
        };
        let context_result = report::generate_report(self, &report_options);

        let _ = thread::Builder::new()
            .name("ai_query".to_string())
            .spawn(move || {
                let result = match context_result {
                    Ok(context) => match Builder::new_current_thread().enable_all().build() {
                        Ok(runtime) => runtime.block_on(gemini_service::query_codebase(
                            &api_key,
                            "gemini-2.5-pro",
                            context,
                            prompt,
                            2.0,
                        )),
                        Err(err) => Err(gemini_service::AppError::Internal(err.to_string())),
                    },
                    Err(err) => Err(gemini_service::AppError::Internal(err.to_string())),
                };

                if task_sender.send(TaskMessage::AIResponse(result)).is_err() {
                    log::warn!("Failed to send AI response result to UI thread.");
                }
            });
    }
}
