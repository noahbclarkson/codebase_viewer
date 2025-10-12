//! Functions for drawing modal dialog windows (Preferences, Report Options, About, Shortcuts).

use crate::{
    app::{AppAction, CodebaseApp},
    report::ReportFormat,
};
use egui::{Button, Color32, Context, DragValue, Grid, RichText, ScrollArea, TextEdit, Window};
use egui_phosphor::regular::*;

/// Draws the Preferences window (modal).
/// Uses a draft copy of the config to allow cancellation.
pub fn draw_preferences_window(app: &mut CodebaseApp, ctx: &Context) {
    if !app.show_preferences_window {
        app.prefs_draft = None;
        return;
    }

    if app.prefs_draft.is_none() {
        app.prefs_draft = Some(app.config.clone());
    }

    let mut save_clicked = false;
    let mut cancel_clicked = false;
    let mut is_open = true;

    let is_dirty = app.prefs_draft.as_ref() != Some(&app.config);

    if let Some(draft) = app.prefs_draft.as_mut() {
        Window::new("Preferences")
            .open(&mut is_open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
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

                            ui.label("Respect .cbvignore:");
                            ui.checkbox(&mut draft.respect_cbvignore, "Use .cbvignore files for custom ignores");
                            ui.end_row();

                            ui.label("Auto-expand Limit:");
                            ui.add(
                                DragValue::new(&mut draft.auto_expand_limit)
                                    .speed(1.0)
                                    .range(0..=10000)
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
                             ui.add(
                                DragValue::new(&mut draft.max_file_size_preview)
                                    .speed(1024.0)
                                    .range(-1..=i64::MAX)
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
                            ui.vertical(|ui| {
                                ui.checkbox(&mut draft.export_include_stats, "Include Statistics Section");
                                ui.checkbox(&mut draft.export_include_contents, "Include Selected File Contents");
                                ui.checkbox(&mut draft.export_include_line_numbers, "Include Line Numbers in File Contents");
                            });
                            ui.end_row();
                        });

                    ui.separator();
                    ui.heading("Integrations");
                    ui.add_space(4.0);
                    Grid::new("prefs_integrations_grid")
                        .num_columns(2)
                        .spacing([40.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Gemini API Key:");
                            let mut key_value = draft.gemini_api_key.clone().unwrap_or_default();
                            let response = ui
                                .add(
                                    egui::TextEdit::singleline(&mut key_value)
                                        .password(true)
                                        .hint_text("Use GEMINI_API_KEY env var to avoid storing locally"),
                                )
                                .on_hover_text(
                                    "Stored locally if provided. Leave blank to rely on GEMINI_API_KEY environment variable.",
                                );
                            if response.changed() {
                                let trimmed = key_value.trim();
                                draft.gemini_api_key = if trimmed.is_empty() {
                                    None
                                } else {
                                    Some(trimmed.to_string())
                                };
                            }
                            ui.end_row();
                        });

                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() { cancel_clicked = true; }
                        if ui.add_enabled(is_dirty, Button::new("Save")).on_hover_text("Save changes").clicked() { save_clicked = true; }
                        if let Ok(config_path) = crate::config::config_file() {
                             ui.label(RichText::new(format!("Config: {}", config_path.display())).small().weak());
                        }
                    });
                });
            });
    }

    if cancel_clicked || !is_open {
        app.prefs_draft = None;
        app.show_preferences_window = false;
        return;
    }

    if save_clicked {
        if let Some(new_cfg) = app.prefs_draft.take() {
            let theme_changed = new_cfg.theme != app.config.theme;
            let hidden_changed = new_cfg.show_hidden_files != app.config.show_hidden_files;
            let cbvignore_changed = new_cfg.respect_cbvignore != app.config.respect_cbvignore;

            app.config = new_cfg;

            if theme_changed {
                CodebaseApp::set_egui_theme(ctx, &app.config.theme);
                app.preview_cache = None;
                if app.show_preview_panel && app.selected_node_id.is_some() {
                    app.trigger_preview_load(app.selected_node_id.unwrap(), ctx);
                }
            }
            if hidden_changed || cbvignore_changed {
                if let Some(root) = app.root_path.clone() {
                    log::info!(
                        "Scan setting changed (hidden files or .cbvignore), triggering rescan of '{}'",
                        root.display()
                    );
                    app.queue_action(AppAction::StartScan(root));
                }
            }

            match app.config.save() {
                Ok(_) => {
                    app.status_message = "Preferences saved successfully.".into();
                    app.show_preferences_window = false;
                }
                Err(e) => {
                    app.status_message = format!("Error saving preferences: {e}");
                    log::error!("Failed to save preferences: {e}");
                    app.prefs_draft = Some(app.config.clone());
                    rfd::MessageDialog::new()
                        .set_level(rfd::MessageLevel::Error)
                        .set_title("Save Preferences Failed")
                        .set_description(format!("Could not save preferences:\n{e}"))
                        .show();
                }
            }
        }
    }
}

pub fn draw_ai_query_window(app: &mut CodebaseApp, ctx: &Context) {
    if !app.show_ai_query_window {
        return;
    }
    let mut is_open = app.show_ai_query_window;

    Window::new("Query Codebase with AI")
        .open(&mut is_open)
        .resizable(true)
        .default_size([620.0, 420.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Enter your question about the selected files:");
            ui.add(
                TextEdit::multiline(&mut app.ai_query_text)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);

            let send_enabled = !app.ai_query_text.trim().is_empty() && !app.is_querying_ai;
            if ui
                .add_enabled(send_enabled, Button::new("Send Query"))
                .on_hover_text("Generate a fresh report context and send it to Gemini")
                .clicked()
            {
                let prompt = app.ai_query_text.trim().to_owned();
                app.queue_action(AppAction::QueryAI(prompt));
            }

            ui.separator();
            ui.heading("Response");
            ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                if app.is_querying_ai {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Awaiting Gemini response...");
                    });
                } else if let Some(response) = &app.ai_response_text {
                    ui.label(response);
                } else {
                    ui.label("(No response yet)");
                }
            });
        });

    if !is_open {
        app.show_ai_query_window = false;
    }
}

// ... (rest of the file is unchanged) ...
pub fn draw_report_options_window(app: &mut CodebaseApp, ctx: &Context) {
    if !app.show_report_options_window {
        app.report_options_draft = None;
        return;
    }

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
                            .selected_text(format!("{:?}", draft.format))
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

                        ui.label("Include Line Numbers:");
                        ui.add_enabled_ui(draft.include_contents, |ui| {
                            ui.checkbox(
                                &mut draft.include_line_numbers,
                                "Prepend line numbers to file content",
                            )
                            .on_hover_text("Only applies if 'Include File Contents' is checked");
                        });
                        ui.end_row();
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            cancel_clicked = true;
                        }
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
            });
    }

    if cancel_clicked || !is_open {
        app.report_options_draft = None;
        app.show_report_options_window = false;
        return;
    }

    if generate_clicked {
        if let Some(opts) = app.report_options_draft.take() {
            app.last_report_options = opts.clone();
            app.queue_action(AppAction::GenerateReport(opts));
            app.show_report_options_window = false;
        }
    }

    if copy_clicked {
        if let Some(opts) = app.report_options_draft.take() {
            app.last_report_options = opts.clone();
            app.queue_action(AppAction::CopyReport(opts));
            app.show_report_options_window = false;
        }
    }
}

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

    if !is_open {
        app.show_about_window = false;
    }
}

pub fn draw_shortcuts_window(app: &mut CodebaseApp, ctx: &Context) {
    if !app.show_shortcuts_window {
        return;
    }
    let mut is_open = app.show_shortcuts_window;

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
        ),
        (
            "Collapse All",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::CloseBracket),
        ),
        (
            "Toggle Preview Panel",
            egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F9),
        ),
        (
            "Preferences",
            egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Comma),
        ),
    ];

    Window::new("Keyboard Shortcuts")
        .open(&mut is_open)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(5.0);
                Grid::new("shortcuts_grid")
                    .num_columns(2)
                    .spacing([30.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Action").strong());
                        ui.label(RichText::new("Shortcut").strong());
                        ui.end_row();

                        for (action, shortcut) in shortcuts {
                            ui.label(action);
                            ui.label(ctx.format_shortcut(&shortcut));
                            ui.end_row();
                        }

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
            });

            ui.separator();
            ui.vertical_centered(|ui| {
                ui.add_space(5.0);
                if ui.button("Close").clicked() {
                    app.show_shortcuts_window = false;
                }
                ui.add_space(5.0);
            });
        });

    if !is_open {
        app.show_shortcuts_window = false;
    }
}
