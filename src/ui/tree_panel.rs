//! Draws the left-hand panel containing the file tree view and controls.

use crate::{
    app::{AppAction, CodebaseApp},
    model::{Check, FileId},
};
// Removed unused Response import
use egui::{Button, Color32, CornerRadius, Id, RichText, ScrollArea, Ui};
use egui_phosphor::regular::*; // Import icons
use once_cell::sync::Lazy;
use std::collections::HashMap;

// --- Static Icon Mapping ---
// Lazily initialized HashMap to map file extensions (lowercase) to Phosphor icon characters.
static ICONS: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        // Programming & Markup
        ("rs", FILE_RS),
        ("toml", GEAR_SIX),
        ("md", FILE_MD),
        ("html", FILE_HTML),
        ("htm", FILE_HTML),
        ("css", FILE_CSS),
        ("js", FILE_JS),
        ("jsx", FILE_JSX),
        ("ts", FILE_TS),
        ("tsx", FILE_TSX),
        ("py", FILE_PY),
        ("java", COFFEE),
        // Use CODE_SIMPLE for C/C++ headers
        ("c", FILE_C),
        ("h", CODE_SIMPLE),
        ("cpp", FILE_CPP),
        ("hpp", CODE_SIMPLE),
        // Use FILE_CODE for C# and PHP as specific icons are missing
        ("cs", FILE_CODE),
        ("go", GOOGLE_LOGO), // Using Google logo as a placeholder for Go Gopher
        ("rb", DIAMOND),
        ("php", FILE_CODE),
        // Use BIRD for Swift, TRIANGLE for Kotlin
        ("swift", BIRD),
        ("kt", TRIANGLE),
        ("json", BRACKETS_CURLY),
        ("yaml", LIST_BULLETS),
        ("yml", LIST_BULLETS),
        ("xml", CODE),
        ("sh", TERMINAL_WINDOW),
        ("bash", TERMINAL_WINDOW),
        ("ps1", TERMINAL),
        ("sql", DATABASE),
        ("lock", LOCK_SIMPLE),
        ("gitignore", GIT_BRANCH),
        ("dockerfile", PACKAGE),
        ("patch", BANDAIDS),
        ("diff", GIT_DIFF),
        // Images
        ("png", IMAGE),
        ("jpg", IMAGE),
        ("jpeg", IMAGE),
        ("gif", GIF),
        ("bmp", IMAGE),
        ("ico", IMAGE_SQUARE),
        ("tiff", IMAGE),
        ("svg", FILE_SVG),
        // Documents
        ("pdf", FILE_PDF),
        ("txt", FILE_TEXT),
        ("doc", FILE_DOC),
        ("docx", MICROSOFT_WORD_LOGO),
        ("xls", MICROSOFT_EXCEL_LOGO),
        ("xlsx", MICROSOFT_EXCEL_LOGO),
        ("ppt", MICROSOFT_POWERPOINT_LOGO),
        ("pptx", MICROSOFT_POWERPOINT_LOGO),
        // Archives
        ("zip", FILE_ZIP),
        ("rar", FILE_ARCHIVE),
        ("gz", FILE_ARCHIVE),
        ("tar", FILE_ARCHIVE),
        ("7z", FILE_ARCHIVE),
        // Audio/Video
        ("mp3", FILE_AUDIO),
        ("wav", FILE_AUDIO),
        ("mp4", FILE_VIDEO),
        ("mov", FILE_VIDEO),
        ("avi", FILE_VIDEO),
        // Config / Other
        ("cfg", GEAR),
        ("conf", GEAR),
        ("ini", GEAR),
        ("log", SCROLL),
        ("exe", GEAR_FINE),
        ("dll", GEAR_FINE),
        ("so", GEAR_FINE),
        ("a", GEAR_FINE),
        ("lib", GEAR_FINE),
        ("o", GEAR_FINE),
        // Default handled later (FILE icon)
    ])
});

/// Draws the tree panel UI, including toolbar and the scrollable tree view.
pub fn draw_tree_panel(app: &mut CodebaseApp, ui: &mut Ui) {
    // --- Toolbar Area ---
    ui.vertical(|ui| {
        // Path Display (Show root directory name or full path if no name)
        ui.horizontal(|ui| {
            ui.label(FOLDER_NOTCH_OPEN); // Icon for directory path
            let path_text = match &app.root_path {
                Some(p) => p.file_name().map_or_else(
                    || p.display().to_string(), // Show full path if root has no name (e.g., "/")
                    |n| n.to_string_lossy().into_owned(), // Show directory name
                ),
                None => "No directory selected".to_string(),
            };
            // Use a tooltip for the full path
            ui.label(RichText::new(&path_text).strong()).on_hover_text(
                app.root_path
                    .as_ref()
                    .map_or("", |p| p.to_str().unwrap_or("")),
            );
        });
        ui.add_space(2.0);

        // Tree Control Buttons
        ui.horizontal(|ui| {
            let tree_loaded = app.root_id.is_some();
            let button_size = egui::vec2(20.0, 20.0); // Smaller buttons

            // Use icons for buttons for a cleaner look
            if ui
                .add_enabled(
                    tree_loaded,
                    Button::new(ARROW_ELBOW_DOWN_RIGHT)
                        .small()
                        .min_size(button_size),
                )
                .on_hover_text("Expand All")
                .clicked()
            {
                app.queue_action(AppAction::ExpandAllNodes);
            }
            if ui
                .add_enabled(
                    tree_loaded,
                    Button::new(ARROW_ELBOW_UP_LEFT)
                        .small()
                        .min_size(button_size),
                )
                .on_hover_text("Collapse All")
                .clicked()
            {
                app.queue_action(AppAction::CollapseAllNodes);
            }
            if ui
                .add_enabled(
                    tree_loaded,
                    Button::new(CHECK_SQUARE).small().min_size(button_size),
                )
                .on_hover_text("Select All")
                .clicked()
            {
                app.queue_action(AppAction::SelectAllNodes);
            }
            if ui
                .add_enabled(
                    tree_loaded,
                    Button::new(SQUARE).small().min_size(button_size),
                )
                .on_hover_text("Deselect All")
                .clicked()
            {
                app.queue_action(AppAction::DeselectAllNodes);
            }
        });
        ui.add_space(4.0);

        // Search Box
        ui.horizontal(|ui| {
            ui.label(MAGNIFYING_GLASS); // Search icon
            let search_box_id = Id::new("tree_search_box"); // Unique ID for the search box
                                                            // Keep the response, it's needed for focus logic
            let search_response = ui.add(
                egui::TextEdit::singleline(&mut app.search_text)
                    .hint_text("Search files...")
                    .id(search_box_id) // Assign the ID
                    .desired_width(f32::INFINITY), // Take available width
            );

            // Handle focus request (e.g., from Ctrl+F)
            if app.focus_search_box {
                // Use the response's ID to request focus
                ui.memory_mut(|mem| mem.request_focus(search_response.id));
                app.focus_search_box = false; // Reset the flag
            }

            // Add a clear button if there's text in the search box
            // Collapsed the nested if as suggested by clippy
            if !app.search_text.is_empty()
                && ui
                    .add(Button::new(X_CIRCLE).small().frame(false))
                    .on_hover_text("Clear search")
                    .clicked()
            {
                app.search_text.clear();
                // Optionally clear focus or keep it
            }

            // Log changes for debugging if needed
            // if search_response.changed() {
            //     log::debug!("Search text changed: {}", app.search_text);
            // }
        });
    }); // End Toolbar Area

    ui.separator();

    // --- Tree View Area ---
    ScrollArea::vertical()
        .id_salt(egui::Id::new("tree_scroll_area")) // Unique ID for scroll area state
        .auto_shrink([false; 2]) // Fill available space
        .show(ui, |ui| {
            if let Some(root_id) = app.root_id {
                // Start drawing the tree recursively from the root
                draw_tree_node_recursive(app, ui, root_id);
            } else if app.is_scanning {
                // Show loading indicator if scanning
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Scanning directory...");
                });
            } else {
                // Show placeholder message if no directory is loaded
                ui.centered_and_justified(|ui| {
                    ui.label("Open a directory (File > Open Directory...)");
                });
            }
        }); // End ScrollArea
}

/// Recursively draws a node and its children in the tree view.
/// Handles indentation, icons, selection, expansion, filtering, and context menus.
fn draw_tree_node_recursive(app: &mut CodebaseApp, ui: &mut Ui, node_id: FileId) {
    // Get the node data immutably first
    let node = match app.nodes.get(node_id) {
        Some(n) => n,
        None => {
            log::error!("Attempted to draw invalid node ID: {}", node_id);
            return; // Don't draw if node ID is invalid
        }
    };

    // --- Filtering Logic ---
    // Check if the node itself or any descendant matches the search text
    let is_match = check_search_match_recursive(app, node_id, &app.search_text);
    if !is_match {
        return; // Skip drawing this node and its children if no match found
    }

    // --- Node Data Extraction ---
    // Clone necessary data to avoid borrowing issues later
    let is_dir = node.is_dir();
    let name = node.name().to_string(); // Clone name string
    let node_state = node.state;
    let is_expanded = node.is_expanded;
    let children_ids = node.children.clone(); // Clone children IDs
    let extension = node.info.extension.as_deref().unwrap_or("");

    // --- Icon Selection ---
    let icon = if is_dir {
        // Use different icons for open/closed folders
        if is_expanded {
            FOLDER_OPEN
        } else {
            FOLDER
        }
    } else {
        // Look up icon based on extension, default to generic file icon
        ICONS.get(extension).copied().unwrap_or(FILE)
    };

    // --- Row Layout ---
    // Use a horizontal layout for each node's row
    let _row_response = ui
        .horizontal(|ui| {
            // Assign to _ to silence unused variable warning if response isn't used
            // Indentation (visual only, using spaces) - Adjust space size as needed
            // let indent_level = ui.indentation_level(); // Not directly available, manage manually if needed
            // ui.add_space(indent_level * 10.0); // Example manual indent

            // 1. Checkbox
            let initial_check_state = match node_state {
                Check::Unchecked => false,
                Check::Checked | Check::Partial => true,
            };
            let mut current_check_state = initial_check_state; // Temporary state for the checkbox widget

            // Create the checkbox widget
            let checkbox_response = ui.add(egui::Checkbox::new(&mut current_check_state, ""));

            // Draw the indeterminate state (partial) manually if needed
            if node_state == Check::Partial {
                let rect = checkbox_response.rect;
                // Draw a smaller filled square inside the checkbox bounds
                let smaller_rect = rect.shrink(rect.width() * 0.3);
                ui.painter().rect_filled(
                    smaller_rect,
                    CornerRadius::ZERO, // Sharp corners for the inner square
                    ui.visuals().strong_text_color(), // Use a contrasting color
                );
            }

            // If the checkbox was clicked, queue the action to toggle the state
            if checkbox_response.clicked() {
                app.queue_action(AppAction::ToggleCheckState(node_id));
            }

            // 2. Expand/Collapse Toggle (for directories)
            if is_dir {
                // Use a clickable label with a caret icon
                let toggle_icon = if is_expanded { CARET_DOWN } else { CARET_RIGHT };
                // Make the toggle visually distinct but small
                if ui
                    .selectable_label(false, RichText::new(toggle_icon).size(14.0))
                    .clicked()
                {
                    app.queue_action(AppAction::ToggleExpandState(node_id));
                }
            } else {
                // Add spacing for files to align them with directory names
                ui.label(RichText::new(DOT_OUTLINE).size(14.0).weak()); // Placeholder dot for files
            }

            // 3. Icon and Name Label
            let label_text = format!("{} {}", icon, name);
            let is_selected = app.selected_node_id == Some(node_id);

            // Highlight text if it matches the search query
            let display_text = if !app.search_text.is_empty()
                && name
                    .to_lowercase()
                    .contains(&app.search_text.to_lowercase())
            {
                RichText::new(label_text)
                    .strong()
                    .color(ui.visuals().hyperlink_color) // Use hyperlink color for matches
            } else {
                RichText::new(label_text)
            };

            // Create the main selectable label for the node name
            let label_response = ui.selectable_label(is_selected, display_text);

            // Handle click on the label (selects the node)
            if label_response.clicked() {
                app.selected_node_id = Some(node_id);
                // Preview loading is handled in app.rs based on selection change
            }

            // Handle double-click (toggle expansion for dirs, maybe open file externally?)
            if label_response.double_clicked() {
                if is_dir {
                    app.queue_action(AppAction::ToggleExpandState(node_id));
                } else {
                    // Optional: Open file on double click?
                    // app.queue_action(AppAction::OpenNodeExternally(node_id));
                }
            }

            // 4. Context Menu
            label_response.context_menu(|ui| {
                // Clone the node_id to avoid borrow issues
                let node_id_clone = node_id;

                // Get node data again safely inside the closure
                if let Some(ctx_node) = app.nodes.get(node_id) {
                    let node_name = ctx_node.name().to_string();
                    let node_path = ctx_node.path().display().to_string();
                    let is_expanded = ctx_node.is_expanded;

                    ui.label(RichText::new(node_name).strong());
                    ui.label(RichText::new(node_path).small().weak());
                    ui.separator();

                    if ui.button("Toggle Selection").clicked() {
                        app.queue_action(AppAction::ToggleCheckState(node_id_clone));
                        if ui
                            .button(if is_expanded {
                                "Collapse Node"
                            } else {
                                "Expand Node"
                            })
                            .clicked()
                        {
                            app.queue_action(AppAction::ToggleExpandState(node_id_clone));
                            ui.close_menu();
                        }
                        if ui.button("Select All Children").clicked() {
                            app.queue_action(AppAction::SelectAllChildren(node_id_clone));
                            ui.close_menu();
                        }
                        if ui.button("Deselect All Children").clicked() {
                            app.queue_action(AppAction::DeselectAllChildren(node_id_clone));
                            ui.close_menu();
                        }
                    } else {
                        // Action specific to files
                        if ui.button("Preview File").clicked() {
                            app.selected_node_id = Some(node_id_clone);
                            // Preview loading handled by selection change logic in app.rs
                            app.show_preview_panel = true; // Ensure preview panel is visible
                            ui.close_menu();
                        }
                    }

                    ui.separator();
                    if ui.button("Open Externally").clicked() {
                        app.queue_action(AppAction::OpenNodeExternally(node_id_clone));
                        ui.close_menu();
                    }
                    if ui.button("Open Externally").clicked() {
                        app.queue_action(AppAction::OpenNodeExternally(node_id));
                        ui.close_menu();
                    }
                } else {
                    ui.label(RichText::new("Error: Node data unavailable").color(Color32::RED));
                }
            }); // End context menu
        })
        .response; // Get the response of the horizontal layout

    // --- Draw Children Recursively ---
    // If the node is an expanded directory, draw its children indented
    if is_dir && is_expanded {
        // Use egui's indentation helper
        ui.indent(format!("indent_{}", node_id), |ui| {
            if children_ids.is_empty() {
                // Optionally indicate empty directories
                // ui.label(RichText::new(" (empty)").weak().small());
            } else {
                // Recursively draw each child node
                for child_id in children_ids {
                    draw_tree_node_recursive(app, ui, child_id);
                }
            }
        });
    }
}

/// Helper function to check if a node or any of its descendants match the search text (case-insensitive).
/// Used for filtering the tree view.
fn check_search_match_recursive(app: &CodebaseApp, node_id: FileId, search_text: &str) -> bool {
    // If search text is empty, everything matches
    if search_text.is_empty() {
        return true;
    }

    // Normalize search text once
    let lower_search = search_text.to_lowercase();

    // Check the current node
    if let Some(node) = app.nodes.get(node_id) {
        // Check if the node name contains the search text
        if node.name().to_lowercase().contains(&lower_search) {
            return true;
        }

        // If it's a directory, check if any children match recursively
        if node.is_dir() {
            return node.children.iter().any(|&child_id| {
                // Short-circuit recursion if a match is found
                check_search_match_recursive(app, child_id, &lower_search) // Pass normalized search text down
            });
        }
    }

    // No match found for this node or its descendants
    false
}
