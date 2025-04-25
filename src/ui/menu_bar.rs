//! Draws the main menu bar at the top of the application window.

use crate::{
    app::{AppAction, CodebaseApp},
    config, // Use config for MAX_RECENT_PROJECTS
    external,
};
use egui::{Context, Key, Modifiers, TopBottomPanel};
use std::path::PathBuf;

/// Draws the top menu bar using `egui::menu::bar`.
pub fn draw_menu_bar(app: &mut CodebaseApp, ctx: &Context) {
    TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            // --- File Menu ---
            ui.menu_button("File", |ui| {
                // Open Directory
                let open_shortcut = ui
                    .ctx()
                    .format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::O));
                if ui
                    .button(format!("Open Directory... ({})", open_shortcut))
                    .clicked()
                {
                    ui.close_menu();
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        app.queue_action(AppAction::StartScan(path));
                    }
                }

                // Recent Projects Submenu
                let recent_enabled = !app.config.recent_projects.is_empty();
                ui.add_enabled_ui(recent_enabled, |ui| {
                    ui.menu_button("Recent Projects", |ui| {
                        if app.config.recent_projects.is_empty() {
                            // Should be disabled by add_enabled_ui, but check anyway
                            ui.label("(No recent projects)");
                        } else {
                            let mut path_to_open: Option<PathBuf> = None;
                            // Iterate over a clone to avoid borrowing issues if clearing
                            let recent_projects = app.config.recent_projects.clone();
                            for (i, path) in recent_projects
                                .iter()
                                .take(config::MAX_RECENT_PROJECTS)
                                .enumerate()
                            {
                                // Attempt to create a concise display label
                                let display_path = path.display().to_string();
                                let label = path
                                    .file_name()
                                    .map(|name| name.to_string_lossy().to_string())
                                    .unwrap_or_else(|| {
                                        // Fallback for paths like "/" or "C:\"
                                        if display_path.len() > 40 {
                                            format!(
                                                "...{}",
                                                &display_path[display_path.len() - 37..]
                                            )
                                        } else {
                                            display_path.clone()
                                        }
                                    });
                                // Add number shortcut hint (1-9, 0)
                                let shortcut_num = match i {
                                    0..=8 => (i + 1).to_string(),
                                    9 => "0".to_string(),
                                    _ => "".to_string(),
                                };
                                let button_text = if shortcut_num.is_empty() {
                                    label
                                } else {
                                    format!("{}. {}", shortcut_num, label)
                                };

                                if ui
                                    .button(button_text)
                                    .on_hover_text(&display_path)
                                    .clicked()
                                {
                                    path_to_open = Some(path.clone());
                                    ui.close_menu(); // Close outer menu as well
                                }
                            }
                            ui.separator();
                            if ui.button("Clear Recent Projects").clicked() {
                                app.config.clear_recent_projects();
                                // Attempt to save config change immediately
                                if let Err(e) = app.config.save() {
                                    log::error!(
                                        "Failed to save config after clearing recent projects: {}",
                                        e
                                    );
                                    app.status_message = format!("Error saving config: {}", e);
                                }
                                ui.close_menu();
                            }
                            // If a recent project was clicked, queue the scan action
                            if let Some(path) = path_to_open {
                                app.queue_action(AppAction::StartScan(path));
                            }
                        }
                    })
                    .response // Access the response of the menu button itself
                    .on_hover_text("Open a recently used directory");
                }); // End Recent Projects submenu

                ui.separator();

                // Save/Load Selection
                let selection_enabled = app.root_path.is_some(); // Enable only if a directory is open
                let save_shortcut = ui
                    .ctx()
                    .format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::S));
                if ui
                    .add_enabled(
                        selection_enabled,
                        egui::Button::new(format!("Save Selection... ({})", save_shortcut)),
                    )
                    .clicked()
                {
                    ui.close_menu();
                    app.queue_action(AppAction::SaveSelection);
                }
                let load_shortcut = ui
                    .ctx()
                    .format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::L));
                if ui
                    .add_enabled(
                        selection_enabled,
                        egui::Button::new(format!("Load Selection... ({})", load_shortcut)),
                    )
                    .clicked()
                {
                    ui.close_menu();
                    app.queue_action(AppAction::LoadSelection);
                }

                ui.separator();

                // Generate Report
                // Disable if no directory is open OR if a scan/report task is already running
                let report_enabled =
                    app.root_path.is_some() && !app.is_scanning && !app.is_generating_report;
                let report_shortcut = ui
                    .ctx()
                    .format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::G));
                if ui
                    .add_enabled(
                        report_enabled,
                        egui::Button::new(format!("Generate Report... ({})", report_shortcut)),
                    )
                    .clicked()
                {
                    ui.close_menu();
                    app.show_report_options_window = true; // Open the options dialog first
                }

                ui.separator();

                // Exit
                let exit_shortcut = ui
                    .ctx()
                    .format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Q)); // Or Alt+F4
                if ui.button(format!("Exit ({})", exit_shortcut)).clicked() {
                    // Request the window to close via eframe's viewport command
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }); // End File Menu

            // --- Edit Menu ---
            ui.menu_button("Edit", |ui| {
                let tree_loaded = app.root_id.is_some(); // Enable actions only if tree is loaded

                let select_all_shortcut = ui
                    .ctx()
                    .format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::A));
                if ui
                    .add_enabled(
                        tree_loaded,
                        egui::Button::new(format!("Select All ({})", select_all_shortcut)),
                    )
                    .clicked()
                {
                    ui.close_menu();
                    app.queue_action(AppAction::SelectAllNodes);
                }

                let deselect_all_shortcut = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
                    Modifiers::COMMAND.plus(Modifiers::SHIFT),
                    Key::A,
                ));
                if ui
                    .add_enabled(
                        tree_loaded,
                        egui::Button::new(format!("Deselect All ({})", deselect_all_shortcut)),
                    )
                    .clicked()
                {
                    ui.close_menu();
                    app.queue_action(AppAction::DeselectAllNodes);
                }

                ui.separator();

                // Find in Tree
                let find_shortcut = ui
                    .ctx()
                    .format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::F));
                if ui
                    .add_enabled(
                        tree_loaded,
                        egui::Button::new(format!("Find... ({})", find_shortcut)),
                    )
                    .clicked()
                {
                    ui.close_menu();
                    app.queue_action(AppAction::FocusSearchBox); // Queue action to focus search box
                }

                ui.separator();

                // Preferences
                let prefs_shortcut = ui
                    .ctx()
                    .format_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma));
                if ui
                    .button(format!("Preferences... ({})", prefs_shortcut))
                    .clicked()
                {
                    ui.close_menu();
                    app.show_preferences_window = true;
                }
            }); // End Edit Menu

            // --- View Menu ---
            ui.menu_button("View", |ui| {
                let tree_loaded = app.root_id.is_some();

                let expand_shortcut = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
                    Modifiers::COMMAND,
                    Key::OpenBracket,
                ));
                if ui
                    .add_enabled(
                        tree_loaded,
                        egui::Button::new(format!("Expand All ({})", expand_shortcut)),
                    )
                    .clicked()
                {
                    ui.close_menu();
                    app.queue_action(AppAction::ExpandAllNodes);
                }

                let collapse_shortcut = ui.ctx().format_shortcut(&egui::KeyboardShortcut::new(
                    Modifiers::COMMAND,
                    Key::CloseBracket,
                ));
                if ui
                    .add_enabled(
                        tree_loaded,
                        egui::Button::new(format!("Collapse All ({})", collapse_shortcut)),
                    )
                    .clicked()
                {
                    ui.close_menu();
                    app.queue_action(AppAction::CollapseAllNodes);
                }

                ui.separator();

                // Toggle Preview Panel
                let toggle_preview_shortcut = ui
                    .ctx()
                    .format_shortcut(&egui::KeyboardShortcut::new(Modifiers::NONE, Key::F9));
                if ui
                    .checkbox(
                        &mut app.show_preview_panel,
                        format!("Show Preview Panel ({})", toggle_preview_shortcut),
                    )
                    .clicked()
                {
                    // State change handled by checkbox binding
                    ui.close_menu();
                }

                ui.separator();

                // Theme Selection Combo Box (within the menu)
                let current_theme_str = app.config.theme.clone(); // Clone for display
                egui::ComboBox::from_label("Theme")
                    .selected_text(current_theme_str.to_uppercase())
                    .show_ui(ui, |ui| {
                        let mut theme_changed = false;
                        theme_changed |= ui
                            .selectable_value(&mut app.config.theme, "light".to_string(), "Light")
                            .changed();
                        theme_changed |= ui
                            .selectable_value(&mut app.config.theme, "dark".to_string(), "Dark")
                            .changed();
                        theme_changed |= ui
                            .selectable_value(&mut app.config.theme, "system".to_string(), "System")
                            .changed();

                        if theme_changed {
                            log::info!("Theme changed via menu to: {}", app.config.theme);
                            // Apply the theme change immediately
                            CodebaseApp::set_egui_theme(ctx, &app.config.theme);
                            // Attempt to save the config change
                            if let Err(e) = app.config.save() {
                                log::error!("Failed to save config after theme change: {}", e);
                                app.status_message = format!("Error saving config: {}", e);
                            }
                            // Invalidate preview cache as theme affects highlighting
                            app.preview_cache = None;
                            if app.show_preview_panel && app.selected_node_id.is_some() {
                                // Trigger reload of current preview if visible
                                app.load_preview_for_node(app.selected_node_id.unwrap(), ctx);
                            }
                            ui.close_menu(); // Close menu after selection
                        }
                    });

                ui.separator();

                // Show Hidden Files Checkbox
                if ui
                    .checkbox(&mut app.config.show_hidden_files, "Show Hidden Files")
                    .changed()
                {
                    log::info!(
                        "Show hidden files toggled via menu to: {}",
                        app.config.show_hidden_files
                    );
                    // If a directory is loaded, trigger a rescan
                    if let Some(root_path) = app.root_path.clone() {
                        app.queue_action(AppAction::StartScan(root_path));
                    }
                    // Attempt to save the config change
                    if let Err(e) = app.config.save() {
                        log::error!("Failed to save config after hidden file toggle: {}", e);
                        app.status_message = format!("Error saving config: {}", e);
                    }
                    ui.close_menu();
                }
            }); // End View Menu

            // --- Help Menu ---
            ui.menu_button("Help", |ui| {
                // Link to repository or documentation website
                let doc_url = "https://github.com/noahbclarkson/codebase_viewer";
                if ui.button("Documentation / Source Code").clicked() {
                    ui.close_menu();
                    if let Err(e) = external::open_path_in_external_app(doc_url.as_ref()) {
                        log::error!("Failed to open documentation URL '{}': {}", doc_url, e);
                        app.status_message = format!("Error opening URL: {}", e);
                        // Show dialog on failure
                        rfd::MessageDialog::new()
                            .set_level(rfd::MessageLevel::Error)
                            .set_title("Open URL Error")
                            .set_description(format!(
                                "Could not open the documentation URL:\n{}",
                                e
                            ))
                            .show();
                    }
                }

                // Show Shortcuts Window
                if ui.button("Keyboard Shortcuts").clicked() {
                    ui.close_menu();
                    app.show_shortcuts_window = true;
                }

                ui.separator();

                // Show About Window
                if ui.button("About").clicked() {
                    ui.close_menu();
                    app.show_about_window = true;
                }
            }); // End Help Menu
        }); // End menu bar
    }); // End TopBottomPanel
}
