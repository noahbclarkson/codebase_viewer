//! Draws the right-hand panel displaying the file preview content.

use crate::{app::CodebaseApp, preview};
use egui::{Color32, Layout, RichText, ScrollArea, Ui};
use egui_phosphor::regular::*;

/// Draws the file preview panel.
pub fn draw_preview_panel(app: &mut CodebaseApp, ui: &mut Ui) {
    // --- Panel Header ---
    ui.horizontal(|ui| {
        ui.heading("File Preview");
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            let close_button = ui
                .add(egui::Button::new(RichText::new(X).size(16.0)).frame(false))
                .on_hover_text("Hide Preview Panel (F9)");
            if close_button.clicked() {
                app.show_preview_panel = false;
            }
        });
    });
    ui.separator();

    // --- Content Area ---
    ScrollArea::vertical()
        .id_salt(egui::Id::new("preview_scroll_area"))
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            if let Some(selected_id) = app.selected_node_id {
                if let Some(node) = app.nodes.get(selected_id) {
                    // Display File Path and Basic Info Header
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(node.path().display().to_string())
                                .weak()
                                .small(),
                        );
                    });
                    let modified_str = node
                        .info
                        .modified
                        .map(|st| {
                            let datetime: chrono::DateTime<chrono::Local> = st.into();
                            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
                        })
                        .unwrap_or_else(|| "N/A".to_string());
                    ui.label(
                        RichText::new(format!(
                            "Size: {} | Modified: {}",
                            node.info.human_size, modified_str
                        ))
                        .small()
                        .color(ui.visuals().text_color().gamma_multiply(0.7)),
                    );
                    ui.separator();
                    ui.add_space(4.0);

                    // Display Preview Content based on cache state
                    match &app.preview_cache {
                        Some(cache_mutex) => {
                            // MODIFIED: Match on Option, not Result
                            match cache_mutex.try_lock() {
                                Some(cache) => {
                                    if cache.node_id == selected_id {
                                        preview::render_preview_content(ui, &cache.content);
                                    } else {
                                        ui.horizontal(|ui| {
                                            ui.spinner();
                                            ui.label("Loading preview...");
                                        });
                                    }
                                }
                                None => {
                                    // MODIFIED: Changed from Err(_)
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label("Generating preview...");
                                    });
                                }
                            }
                        }
                        None => {
                            if node.is_dir() {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(ui.available_height() * 0.3);
                                    ui.label(
                                        RichText::new(FOLDER_OPEN)
                                            .size(48.0)
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                    ui.label(RichText::new("Directory Selected").strong());
                                });
                            } else {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label("Loading preview...");
                                });
                            }
                        }
                    }
                } else {
                    ui.colored_label(Color32::RED, "Error: Selected node data not found.");
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a file from the tree to preview.");
                });
            }
        });
}
