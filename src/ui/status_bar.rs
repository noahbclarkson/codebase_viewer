//! Draws the status bar at the bottom of the application window.

use crate::app::{AppAction, CodebaseApp};
use egui::{Align, Context, Layout, RichText, TopBottomPanel};

/// Draws the bottom status bar, showing messages, scan progress, and file counts.
pub fn draw_status_bar(app: &mut CodebaseApp, ctx: &Context) {
    TopBottomPanel::bottom("status_bar")
        .show_separator_line(true) // Add a visual line above the status bar
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // --- Left Aligned: Status Message ---
                // Use a label that can wrap if the message is long
                // Use a tooltip to show the full message if it's truncated visually
                let _ = ui
                    .label(app.status_message.as_str())
                    .on_hover_text(&app.status_message);

                // Allocate remaining space to the right-aligned elements
                ui.allocate_ui_with_layout(
                    ui.available_size_before_wrap(), // Use available horizontal space
                    Layout::right_to_left(Align::Center), // Align elements to the right
                    |ui| {
                        // --- Right Aligned Elements ---

                        // File Counts (Selected / Total)
                        let (total_files, selected_files) = app.count_files(); // Use helper method
                        ui.label(format!("{selected_files} / {total_files}"))
                            .on_hover_text("Selected Files / Total Files");
                        ui.separator();

                        // Scan Statistics Summary (if available)
                        if let Some(stats) = &app.scan_stats {
                            ui.label(stats.total_size_human().to_string())
                                .on_hover_text("Total size of scanned files");
                            ui.separator();
                            ui.label(format!("{} Dirs", stats.total_dirs))
                                .on_hover_text("Total scanned directories");
                            ui.separator();
                            // Total files already shown above
                            // ui.label(format!("{} Files", stats.total_files));
                            // ui.separator();
                        }

                        // Spinner and Cancel Button (if scanning)
                        if app.is_scanning {
                            if ui.button("Cancel Scan").clicked() {
                                app.queue_action(AppAction::CancelScan); // Queue cancellation
                            }
                            ui.spinner(); // Show spinner last (far right)
                            ui.separator();
                        }
                        // Spinner (if generating report) - No cancel button for now
                        else if app.is_generating_report {
                            ui.label(RichText::new("Generating Report...").small().weak());
                            ui.spinner();
                            ui.separator();
                        }
                    },
                ); // End right-to-left layout
            }); // End horizontal layout
        }); // End TopBottomPanel
}
