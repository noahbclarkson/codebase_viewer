//! Defines the main application state (`CodebaseApp`) and related data structures.

use crate::{
    config::AppConfig,
    fs::ScanStats,
    model::{FileId, FileNode},
    preview::PreviewCache,
    report::ReportOptions,
    task::{ScanMessage, TaskMessage},
};
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

/// Represents the types of background tasks the application can run.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum BackgroundTask {
    Scan(JoinHandle<()>, Arc<AtomicBool>),
    Report(JoinHandle<()>),
}

/// The main application struct, holding all state.
pub struct CodebaseApp {
    // --- Configuration ---
    pub(crate) config: AppConfig,

    // --- UI State ---
    pub(crate) selected_node_id: Option<FileId>,
    pub(crate) search_text: String,
    pub(crate) status_message: String,
    pub(crate) preview_cache: Option<Arc<Mutex<PreviewCache>>>,
    pub(crate) show_preview_panel: bool,
    pub(crate) preview_word_wrap: bool, // MODIFIED: Added word wrap state
    pub(crate) show_preferences_window: bool,
    pub(crate) show_report_options_window: bool,
    pub(crate) show_about_window: bool,
    pub(crate) show_shortcuts_window: bool,
    pub(crate) show_ai_query_window: bool,
    pub(crate) ai_query_text: String,
    pub(crate) ai_response_text: Option<String>,
    pub(crate) is_querying_ai: bool,
    pub(crate) last_report_options: ReportOptions,
    pub(crate) focus_search_box: bool,
    pub(crate) prefs_draft: Option<AppConfig>,
    pub(crate) report_options_draft: Option<ReportOptions>,

    // --- Data State ---
    pub(crate) nodes: Vec<FileNode>,
    pub(crate) root_id: Option<FileId>,
    pub(crate) root_path: Option<PathBuf>,
    pub(crate) scan_stats: Option<ScanStats>,
    pub(crate) orphaned_children: HashMap<PathBuf, Vec<(FileId, PathBuf)>>,
    pub(crate) path_to_id_map: HashMap<PathBuf, FileId>,

    // --- Background Task State ---
    pub(crate) scan_receiver: Option<Receiver<ScanMessage>>,
    pub(crate) preview_receiver: Option<Receiver<(FileId, PreviewCache)>>,
    pub(crate) preview_sender: Option<Sender<(FileId, PreviewCache)>>,
    pub(crate) task_receiver: Option<Receiver<TaskMessage>>,
    pub(crate) task_sender: Option<Sender<TaskMessage>>,
    pub(crate) background_task: Option<BackgroundTask>,
    pub(crate) is_scanning: bool,
    pub(crate) is_generating_report: bool,

    // --- Syntax Highlighting Assets ---
    pub(crate) syntax_set: &'static SyntaxSet,
    pub(crate) theme_set: &'static ThemeSet,

    // --- UI Action Deferral ---
    pub(crate) deferred_actions: Vec<super::AppAction>,
}

impl CodebaseApp {
    /// Creates a new instance of the `CodebaseApp`.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        syntax_set: &'static SyntaxSet,
        theme_set: &'static ThemeSet,
    ) -> Self {
        let config = AppConfig::load();
        Self::set_egui_theme(&cc.egui_ctx, &config.theme);

        let (preview_tx, preview_rx) = crossbeam_channel::unbounded();
        let (task_tx, task_rx) = crossbeam_channel::unbounded();

        let last_report_options = ReportOptions::from_config(&config);

        Self {
            config,
            last_report_options,
            selected_node_id: None,
            search_text: String::new(),
            status_message: "Ready. Select a directory to open (File > Open Directory)."
                .to_string(),
            preview_cache: None,
            show_preview_panel: true,
            preview_word_wrap: false, // MODIFIED: Initialize word wrap to false
            show_preferences_window: false,
            show_report_options_window: false,
            show_about_window: false,
            show_shortcuts_window: false,
            show_ai_query_window: false,
            ai_query_text: String::new(),
            ai_response_text: None,
            is_querying_ai: false,
            focus_search_box: false,
            nodes: Vec::new(),
            root_id: None,
            root_path: None,
            scan_stats: Some(ScanStats::default()),
            scan_receiver: None,
            preview_receiver: Some(preview_rx),
            preview_sender: Some(preview_tx),
            task_receiver: Some(task_rx),
            task_sender: Some(task_tx),
            background_task: None,
            is_scanning: false,
            is_generating_report: false,
            syntax_set,
            theme_set,
            deferred_actions: Vec::new(),
            orphaned_children: HashMap::new(),
            path_to_id_map: HashMap::new(),
            prefs_draft: None,
            report_options_draft: None,
        }
    }

    pub fn headless_from_config(config: AppConfig) -> Self {
        let (syntax_set, theme_set) = crate::preview::load_syntax_highlighting_assets();
        let last_report_options = ReportOptions::from_config(&config);

        Self {
            config,
            selected_node_id: None,
            search_text: String::new(),
            status_message: String::new(),
            preview_cache: None,
            show_preview_panel: false,
            preview_word_wrap: false,
            show_preferences_window: false,
            show_report_options_window: false,
            show_about_window: false,
            show_shortcuts_window: false,
            show_ai_query_window: false,
            ai_query_text: String::new(),
            ai_response_text: None,
            is_querying_ai: false,
            last_report_options,
            focus_search_box: false,
            prefs_draft: None,
            report_options_draft: None,
            nodes: Vec::new(),
            root_id: None,
            root_path: None,
            scan_stats: Some(ScanStats::default()),
            orphaned_children: HashMap::new(),
            path_to_id_map: HashMap::new(),
            scan_receiver: None,
            preview_receiver: None,
            preview_sender: None,
            task_receiver: None,
            task_sender: None,
            background_task: None,
            is_scanning: false,
            is_generating_report: false,
            syntax_set,
            theme_set,
            deferred_actions: Vec::new(),
        }
    }
    /// Queues an action to be processed after the current UI update cycle.
    pub(crate) fn queue_action(&mut self, action: super::AppAction) {
        self.deferred_actions.push(action);
    }

    /// Saves the current configuration to disk.
    pub(crate) fn save_config(&mut self) {
        log::info!("Saving configuration on exit...");
        if let Err(e) = self.config.save() {
            log::error!("Failed to save config on exit: {e}");
        }
    }

    /// Cleans up background tasks on application exit.
    pub(crate) fn cleanup_on_exit(&mut self) {
        log::info!("Exit requested. Cleaning up background tasks...");
        if let Some(BackgroundTask::Scan(_, cancel)) = &self.background_task {
            log::info!("Requesting scan cancellation on exit...");
            cancel.store(true, Ordering::Relaxed);
        }
        if self.background_task.take().is_some() {
            log::info!("A background task was running during exit.");
        }
    }
}
