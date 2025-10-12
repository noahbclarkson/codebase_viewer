//! # Application Module
//!
//! This module orchestrates the entire application, implementing the `eframe::App` trait.
//! It is broken down into sub-modules for clarity:
//! - `state`: Defines the main `CodebaseApp` struct and its state.
//! - `actions`: Implements the logic for all `AppAction` variants.
//! - `message_handling`: Processes messages from background threads.
//! - `helpers`: Contains miscellaneous helper functions for the app.

use crate::ui;
use egui::{Context, Key, Modifiers};

// Make sub-modules accessible within the `app` module
mod actions;
mod helpers;
mod message_handling;
mod report_preview;
pub mod state;

// Re-export the main app struct and action enum for easier use in other modules.
pub use state::CodebaseApp;

/// Represents actions that modify the application state, often triggered by UI events.
/// These are queued in `deferred_actions` and processed after the UI draw pass.
#[derive(Debug)]
pub(crate) enum AppAction {
    ToggleCheckState(crate::model::FileId),
    ToggleExpandState(crate::model::FileId),
    SelectAllNodes,
    DeselectAllNodes,
    ExpandAllNodes,
    CollapseAllNodes,
    SelectAllChildren(crate::model::FileId),
    DeselectAllChildren(crate::model::FileId),
    OpenNodeExternally(crate::model::FileId),
    SaveSelection,
    LoadSelection,
    GenerateReport(crate::report::ReportOptions),
    CopyReport(crate::report::ReportOptions),
    StartScan(std::path::PathBuf),
    CancelScan,
    FocusSearchBox,
    QueryAI(String),
}

impl eframe::App for CodebaseApp {
    /// Called each frame to update the application state and draw the UI.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // 1. Handle messages from background tasks (scanner, report generator)
        self.handle_background_messages();

        // 2. Handle global keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx);

        // 3. Draw all UI elements
        self.draw_ui(ctx);

        // 4. Process any actions that were queued during the UI pass
        let had_actions = !self.deferred_actions.is_empty();
        self.process_deferred_actions();

        // 5. Request a repaint if needed to keep the UI responsive
        if self.is_scanning || self.is_generating_report || had_actions || self.focus_search_box {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    /// Called when the application is about to close.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.save_config();
    }

    /// Called just before the application exits for cleanup.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cleanup_on_exit();
    }
}

impl CodebaseApp {
    /// Central place to draw all primary UI components.
    fn draw_ui(&mut self, ctx: &Context) {
        ui::draw_menu_bar(self, ctx);
        ui::draw_status_bar(self, ctx);

        egui::SidePanel::left("tree_panel")
            .resizable(true)
            .default_width(350.0)
            .width_range(200.0..=800.0)
            .show(ctx, |ui| {
                let selected_before = self.selected_node_id;
                ui::draw_tree_panel(self, ui);
                let selected_after = self.selected_node_id;

                if selected_before != selected_after {
                    if let Some(new_id) = selected_after {
                        self.trigger_preview_load(new_id, ctx);
                    } else {
                        self.preview_cache = None;
                    }
                }
            });

        if self.show_preview_panel {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui::draw_preview_panel(self, ui);
            });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label("Preview panel hidden (View > Show Preview Panel or F9)");
                });
            });
        }

        ui::draw_preferences_window(self, ctx);
        ui::draw_report_options_window(self, ctx);
        ui::draw_ai_query_window(self, ctx);
        ui::draw_about_window(self, ctx);
        ui::draw_shortcuts_window(self, ctx);
    }

    /// Central place to handle global keyboard shortcuts.
    fn handle_keyboard_shortcuts(&mut self, ctx: &Context) {
        if ctx.wants_keyboard_input() {
            return; // Don't process global shortcuts if a text field has focus
        }

        let open_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
        let save_sel_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
        let load_sel_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::L);
        let report_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::G);
        let select_all_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::A);
        let deselect_all_shortcut =
            egui::KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::A);
        let find_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::F);
        let expand_all_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::OpenBracket);
        let collapse_all_shortcut =
            egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::CloseBracket);
        let toggle_preview_shortcut = egui::KeyboardShortcut::new(Modifiers::NONE, Key::F9);
        let prefs_shortcut = egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma);

        ctx.input_mut(|i| {
            if i.consume_shortcut(&open_shortcut) {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.queue_action(AppAction::StartScan(path));
                }
            } else if i.consume_shortcut(&save_sel_shortcut) && self.root_path.is_some() {
                self.queue_action(AppAction::SaveSelection);
            } else if i.consume_shortcut(&load_sel_shortcut) && self.root_path.is_some() {
                self.queue_action(AppAction::LoadSelection);
            } else if i.consume_shortcut(&report_shortcut)
                && self.root_path.is_some()
                && !self.is_scanning
                && !self.is_generating_report
            {
                self.show_report_options_window = true;
            } else if i.consume_shortcut(&select_all_shortcut) && self.root_id.is_some() {
                self.queue_action(AppAction::SelectAllNodes);
            } else if i.consume_shortcut(&deselect_all_shortcut) && self.root_id.is_some() {
                self.queue_action(AppAction::DeselectAllNodes);
            } else if i.consume_shortcut(&find_shortcut) && self.root_id.is_some() {
                self.queue_action(AppAction::FocusSearchBox);
            } else if i.consume_shortcut(&expand_all_shortcut) && self.root_id.is_some() {
                self.queue_action(AppAction::ExpandAllNodes);
            } else if i.consume_shortcut(&collapse_all_shortcut) && self.root_id.is_some() {
                self.queue_action(AppAction::CollapseAllNodes);
            } else if i.consume_shortcut(&toggle_preview_shortcut) {
                self.show_preview_panel = !self.show_preview_panel;
            } else if i.consume_shortcut(&prefs_shortcut) {
                self.show_preferences_window = true;
            }
        });
    }
}
