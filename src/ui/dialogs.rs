//! Functions for drawing modal dialog windows (Preferences, Report Options, About, Shortcuts).

use crate::{
    app::{AppAction, CodebaseApp},
    report::ReportFormat,
};
use egui::{Button, Color32, Context, DragValue, Grid, RichText, ScrollArea, Window};
use egui_phosphor::regular::*; // Import icons

/// Draws the Preferences window (modal).
/// Uses a draft copy of the config to allow cancellation.
pub fn draw_preferences_window(app: &mut CodebaseApp, ctx: &Context) {
    // Only proceed if the window should be shown
    if !app.show_preferences_window {
        app.prefs_draft = None; // Ensure draft is cleared when window is not shown
        return;
    }

    // Initialize the draft config the first time the window is opened
    if app.prefs_draft.is_none() {
        app.prefs_draft = Some(app.config.clone());
    }

    // Use flags to track button clicks within the window closure
    let mut save_clicked = false;
    let mut cancel_clicked = false;
    let mut is_open = true; // Controls window visibility via `open()`

    // Pre-calculate if the draft differs from the current config for enabling the Save button
    let is_dirty = app.prefs_draft.as_ref() != Some(&app.config);

    // Borrow the draft mutably inside a limited scope
    if let Some(draft) = app.prefs_draft.as_mut() {
        Window::new("Preferences")
            .open(&mut is_open) // `is_open` will be set to false if user clicks 'x'
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]) // Center the window
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| { // Allow scrolling if content overflows
                    ui.heading("General");
                    ui.add_space(4.0);
                    Grid::new("prefs_general_grid")
                        .num_columns(2)
                        .spacing([40.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Application Theme:");
                            egui::ComboBox::from_id_salt("theme_combo")
                                .selected_text(format!("{} ({})", draft.theme.to_uppercase(), if draft.theme == "system" { "Auto" } else { "Manual" }))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut draft.theme, "light".into(), "Light");
                                    ui.selectable_value(&mut draft.theme, "dark".into(), "Dark");
                                    ui.selectable_value(&mut draft.theme, "system".into(), "System (Auto)");
                                });
                            ui.end_row();

                            ui.label("Show Hidden Files:");
                            ui.checkbox(&mut draft.show_hidden_files, "Include files/dirs starting with '.'");
                            ui.end_row();

                            ui.label("Auto-expand Limit:");
                            ui.add(
                                // Use range instead of deprecated clamp_range
                                DragValue::new(&mut draft.auto_expand_limit)
                                    .speed(1.0) // Adjust drag speed
                                    .range(0..=10000) // Set reasonable range
                                    .suffix(" files")
                            ).on_hover_text("Automatically expand directories containing fewer than or equal to this many files after scanning.");
                            ui.end_row();
                        });

                    ui.separator();
                    ui.heading("Preview & Export");
                    ui.add_space(4.0);
                    Grid::new("prefs_export_grid")
                        .num_columns(2)
                        .spacing([40.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Max File Size:");
                            // Use DragValue for numeric input
                             ui.add(
                                // Use range instead of deprecated clamp_range
                                DragValue::new(&mut draft.max_file_size_preview)
                                    .speed(1024.0) // Adjust drag speed (bytes)
                                    .range(-1..=i64::MAX) // Allow -1 for unlimited
                                    .prefix("Bytes: ")
                            ).on_hover_text("Maximum size (in bytes) for file preview and inclusion in reports. Set to -1 for unlimited (use with caution).");
                            ui.end_row();

                            ui.label("Default Export Format:");
                            egui::ComboBox::from_id_salt("export_format_combo")
                                .selected_text(draft.export_format.to_uppercase())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut draft.export_format, "markdown".into(), "Markdown");
                                    ui.selectable_value(&mut draft.export_format, "html".into(), "HTML");
                                    ui.selectable_value(&mut draft.export_format, "text".into(), "Text");
                                });
                            ui.end_row();

                            ui.label("Default Export Options:");
                            // Checkboxes for default report content
                            ui.vertical(|ui| {
                                ui.checkbox(&mut draft.export_include_stats, "Include Statistics Section");
                                ui.checkbox(&mut draft.export_include_contents, "Include Selected File Contents");
                            });
                            ui.end_row();
                        });
                }); // End ScrollArea

                ui.separator();
                // Action buttons at the bottom
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() { cancel_clicked = true; }
                        // Use the pre-calculated `is_dirty` flag here
                        if ui.add_enabled(is_dirty, Button::new("Save")).on_hover_text("Save changes").clicked() { save_clicked = true; }
                         // Show original config path for reference
                        if let Ok(config_path) = crate::config::config_file() {
                             ui.label(RichText::new(format!("Config: {}", config_path.display())).small().weak());
                        }
                    });
                });
            }); // End Window
    } // Draft borrow ends here

    // Handle window closure (via 'x' button or Cancel)
    if cancel_clicked || !is_open {
        app.prefs_draft = None; // Discard changes
        app.show_preferences_window = false;
        return;
    }

    // Handle Save button click
    if save_clicked {
        if let Some(new_cfg) = app.prefs_draft.take() {
            // Take ownership of the draft
            let theme_changed = new_cfg.theme != app.config.theme;
            let hidden_changed = new_cfg.show_hidden_files != app.config.show_hidden_files;

            // Apply the new config
            app.config = new_cfg;

            // Apply immediate effects of config changes
            if theme_changed {
                CodebaseApp::set_egui_theme(ctx, &app.config.theme);
                // Invalidate preview cache as theme affects highlighting
                app.preview_cache = None;
                if app.show_preview_panel && app.selected_node_id.is_some() {
                    // Trigger reload of current preview if visible
                    app.load_preview_for_node(app.selected_node_id.unwrap(), ctx);
                }
            }
            // If hidden file setting changed, trigger a rescan if a directory is open
            if hidden_changed {
                if let Some(root) = app.root_path.clone() {
                    log::info!(
                        "Hidden file setting changed, triggering rescan of '{}'",
                        root.display()
                    );
                    app.queue_action(AppAction::StartScan(root));
                }
            }

            // Attempt to save the new config to disk
            match app.config.save() {
                Ok(_) => {
                    app.status_message = "Preferences saved successfully.".into();
                    app.show_preferences_window = false; // Close window on successful save
                }
                Err(e) => {
                    app.status_message = format!("Error saving preferences: {}", e);
                    log::error!("Failed to save preferences: {}", e);
                    // Keep window open so user can see the error and potentially retry
                    // Put the config back into the draft for retry
                    app.prefs_draft = Some(app.config.clone());
                    // Show error dialog
                    rfd::MessageDialog::new()
                        .set_level(rfd::MessageLevel::Error)
                        .set_title("Save Preferences Failed")
                        .set_description(format!("Could not save preferences:\n{}", e))
                        .show();
                }
            }
        }
    }
}

/// Draws the Report Options window (modal) before generation.
/// Uses a draft copy of the options.
pub fn draw_report_options_window(app: &mut CodebaseApp, ctx: &Context) {
    // Only proceed if the window should be shown
    if !app.show_report_options_window {
        app.report_options_draft = None; // Ensure draft is cleared
        return;
    }

    // Initialize draft options based on the last used options
    if app.report_options_draft.is_none() {
        app.report_options_draft = Some(app.last_report_options.clone());
    }

    let mut generate_clicked = false;
    let mut copy_clicked = false;
    let mut cancel_clicked = false;
    let mut is_open = true;

    if let Some(draft) = app.report_options_draft.as_mut() {
        Window::new("Generate Report Options")
            .open(&mut is_open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                Grid::new("report_opts_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Format:");
                        egui::ComboBox::from_id_salt("report_format_combo")
                            .selected_text(format!("{:?}", draft.format)) // Show enum variant name
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut draft.format,
                                    ReportFormat::Markdown,
                                    "Markdown (.md)",
                                );
                                ui.selectable_value(
                                    &mut draft.format,
                                    ReportFormat::Html,
                                    "HTML (.html)",
                                );
                                ui.selectable_value(
                                    &mut draft.format,
                                    ReportFormat::Text,
                                    "Text (.txt)",
                                );
                            });
                        ui.end_row();

                        ui.label("Include Statistics:");
                        ui.checkbox(&mut draft.include_stats, "Add project statistics section");
                        ui.end_row();

                        ui.label("Include File Contents:");
                        ui.checkbox(&mut draft.include_contents, "Add content of selected files");
                        ui.end_row();
                    });

                ui.separator();
                // Action buttons
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            cancel_clicked = true;
                        }
                        // Disable Generate button if a task is already running
                        if ui
                            .add_enabled(
                                !app.is_scanning && !app.is_generating_report,
                                Button::new("Generate"),
                            )
                            .on_hover_text("Generate the report file")
                            .clicked()
                        {
                            generate_clicked = true;
                        }
                        if ui
                            .add_enabled(
                                !app.is_scanning && !app.is_generating_report,
                                Button::new("Copy to Clipboard"),
                            )
                            .on_hover_text("Copy the report to the clipboard")
                            .clicked()
                        {
                            copy_clicked = true;
                        }
                        if app.is_scanning || app.is_generating_report {
                            ui.label(RichText::new("Busy...").color(Color32::RED).small());
                        }
                    });
                });
            }); // End Window
    } // Draft borrow ends

    // Handle window closure (via 'x' or Cancel)
    if cancel_clicked || !is_open {
        app.report_options_draft = None; // Discard changes
        app.show_report_options_window = false;
        return;
    }

    // Handle Generate button click
    if generate_clicked {
        if let Some(opts) = app.report_options_draft.take() {
            // Take ownership of draft
            app.last_report_options = opts.clone(); // Update last used options
            app.queue_action(AppAction::GenerateReport(opts)); // Queue the generation action
            app.show_report_options_window = false; // Close the dialog
        }
    }

    // Handle Copy button click
    if copy_clicked {
        if let Some(opts) = app.report_options_draft.take() {
            app.last_report_options = opts.clone();
            app.queue_action(AppAction::CopyReport(opts));
            app.show_report_options_window = false;
        }
    }
}

/// Draws the About window (modal).
pub fn draw_about_window(app: &mut CodebaseApp, ctx: &Context) {
    if !app.show_about_window {
        return;
    }
    let mut is_open = app.show_about_window;

    Window::new("About Codebase Viewer RS")
        .open(&mut is_open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                // Use a relevant icon if desired, e.g., INFO or CODE
                ui.label(RichText::new(CODE).size(48.0));
                ui.add_space(5.0);
                ui.heading("Codebase Viewer");
                ui.label(format!("Version: {}", env!("CARGO_PKG_VERSION")));
                ui.add_space(10.0);
                ui.label("A tool for exploring and documenting code repositories.");
                ui.label("Written in Rust using the egui library.");
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label("© 2024-2025 Noah B. Clarkson");
                ui.hyperlink_to(
                    "View Source on GitHub",
                    "https://github.com/noahbclarkson/codebase_viewer",
                );
                ui.add_space(15.0);
                if ui.button("Close").clicked() {
                    app.show_about_window = false;
                }
                ui.add_space(10.0);
            });
        });

    // Update state if the window was closed via the 'x' button
    if !is_open {
        app.show_about_window = false;
    }
}

/// Draws the Keyboard Shortcuts help window (modal).
pub fn draw_shortcuts_window(app: &mut CodebaseApp, ctx: &Context) {
    if !app.show_shortcuts_window {
        return;
    }
    let mut is_open = app.show_shortcuts_window;

    // Define shortcuts here for display consistency using egui's cross-platform Modifiers::COMMAND
    let shortcuts = [
        (
            "Open Directory",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O),
        ),
        (
            "Save Selection",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S),
        ),
        (
            "Load Selection",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::L),
        ),
        (
            "Generate Report",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::G),
        ),
        (
            "Select All (Tree)",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::A),
        ),
        (
            "Deselect All (Tree)",
            egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                egui::Key::A,
            ),
        ),
        (
            "Find in Tree",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::F),
        ),
        (
            "Expand All",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::OpenBracket),
        ), // Cmd/Ctrl + [
        (
            "Collapse All",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::CloseBracket),
        ), // Cmd/Ctrl + ]
        (
            "Toggle Preview Panel",
            egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F9),
        ),
        (
            "Preferences",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Comma),
        ),
        // Exit shortcuts are typically handled by OS/window manager, but can be listed
        // ("Exit", egui::KeyboardShortcut::new(egui::Modifiers::ALT, egui::Key::F4)), // Alt+F4 (Win/Linux)
        // ("Exit", egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Q)), // Cmd+Q (Mac) / Ctrl+Q
    ];

    Window::new("Keyboard Shortcuts")
        .open(&mut is_open)
        .resizable(true) // Allow resizing
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(5.0);
                Grid::new("shortcuts_grid")
                    .num_columns(2)
                    .spacing([30.0, 6.0]) // Adjust spacing
                    .striped(true)
                    .show(ui, |ui| {
                        // Grid Header
                        ui.label(RichText::new("Action").strong());
                        ui.label(RichText::new("Shortcut").strong());
                        ui.end_row();

                        // Populate grid with shortcuts
                        for (action, shortcut) in shortcuts {
                            ui.label(action);
                            ui.label(ctx.format_shortcut(&shortcut));
                            ui.end_row();
                        }

                        // Manually add exit shortcuts description if needed
                        ui.label("Exit Application");
                        ui.label(format!(
                            "{} / {}",
                            ctx.format_shortcut(&egui::KeyboardShortcut::new(
                                egui::Modifiers::ALT,
                                egui::Key::F4
                            )),
                            ctx.format_shortcut(&egui::KeyboardShortcut::new(
                                egui::Modifiers::COMMAND,
                                egui::Key::Q
                            ))
                        ));
                        ui.end_row();
                    });
                ui.add_space(5.0);
            }); // End ScrollArea

            ui.separator();
            // Centered Close button
            ui.vertical_centered(|ui| {
                ui.add_space(5.0);
                if ui.button("Close").clicked() {
                    app.show_shortcuts_window = false;
                }
                ui.add_space(5.0);
            });
        });

    // Update state if window closed via 'x'
    if !is_open {
        app.show_shortcuts_window = false;
    }
}
