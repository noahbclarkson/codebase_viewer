//! Draws the left-hand panel containing the file tree view and controls.

use crate::{
    app::{AppAction, CodebaseApp},
    model::{Check, FileId},
};
use egui::{Button, Color32, CornerRadius, Id, RichText, ScrollArea, Ui};
use egui_phosphor::regular::*;
use once_cell::sync::Lazy;
use std::collections::HashMap;

// --- Static Icon Mapping ---
// (This section remains unchanged)
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
        ("c", FILE_C),
        ("h", CODE_SIMPLE),
        ("cpp", FILE_CPP),
        ("hpp", CODE_SIMPLE),
        ("cs", FILE_CODE),
        ("go", GOOGLE_LOGO),
        ("rb", DIAMOND),
        ("php", FILE_CODE),
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
    ])
});

/// A flattened representation of a tree node for virtual scrolling.
struct TreeRow {
    id: FileId,
    depth: usize,
}

/// Draws the tree panel UI, including toolbar and the scrollable tree view.
pub fn draw_tree_panel(app: &mut CodebaseApp, ui: &mut Ui) {
    // --- Toolbar Area ---
    ui.vertical(|ui| {
        // Path Display
        ui.horizontal(|ui| {
            ui.label(FOLDER_NOTCH_OPEN);
            let path_text = match &app.root_path {
                Some(p) => p.file_name().map_or_else(
                    || p.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                ),
                None => "No directory selected".to_string(),
            };
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
            let button_size = egui::vec2(20.0, 20.0);

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
            ui.label(MAGNIFYING_GLASS);
            let search_box_id = Id::new("tree_search_box");
            let search_response = ui.add(
                egui::TextEdit::singleline(&mut app.search_text)
                    .hint_text("Search files...")
                    .id(search_box_id)
                    .desired_width(f32::INFINITY),
            );

            if app.focus_search_box {
                ui.memory_mut(|mem| mem.request_focus(search_response.id));
                app.focus_search_box = false;
            }

            if !app.search_text.is_empty()
                && ui
                    .add(Button::new(X_CIRCLE).small().frame(false))
                    .on_hover_text("Clear search")
                    .clicked()
            {
                app.search_text.clear();
            }
        });
    });

    ui.separator();

    // --- Tree View Area (with Virtual Scrolling) ---
    if let Some(root_id) = app.root_id {
        // 1. Flatten the visible tree into a linear list
        let mut rows = Vec::new();
        flatten_tree(app, root_id, 0, &app.search_text.to_lowercase(), &mut rows);

        // 2. Use `show_rows` for efficient virtual scrolling (handles its own scrolling)
        let row_height = ui.spacing().interact_size.y;
        ScrollArea::vertical().show_rows(ui, row_height, rows.len(), |ui, row_range| {
            for i in row_range {
                if let Some(row) = rows.get(i) {
                    draw_single_row(app, ui, row.id, row.depth);
                }
            }
        });
    } else if app.is_scanning {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Scanning directory...");
        });
    } else {
        ui.centered_and_justified(|ui| {
            ui.label("Open a directory (File > Open Directory...)");
        });
    }
}

/// Recursively flattens the tree into a `Vec<TreeRow>` for virtual scrolling.
/// Only includes nodes that match the search filter.
fn flatten_tree(
    app: &CodebaseApp,
    node_id: FileId,
    depth: usize,
    lower_search: &str,
    rows: &mut Vec<TreeRow>,
) {
    if !check_search_match_recursive(app, node_id, lower_search) {
        return;
    }

    rows.push(TreeRow { id: node_id, depth });

    if let Some(node) = app.nodes.get(node_id) {
        if node.is_dir() && node.is_expanded {
            for &child_id in &node.children {
                flatten_tree(app, child_id, depth + 1, lower_search, rows);
            }
        }
    }
}

/// Draws a single row in the tree view.
/// This function contains the logic previously in `draw_tree_node_recursive`,
/// but without the recursion itself.
fn draw_single_row(app: &mut CodebaseApp, ui: &mut Ui, node_id: FileId, depth: usize) {
    let node = match app.nodes.get(node_id) {
        Some(n) => n,
        None => return,
    };

    // --- Node Data Extraction ---
    let is_dir = node.is_dir();
    let name = node.name().to_string();
    let node_state = node.state;
    let is_expanded = node.is_expanded;
    let extension = node.info.extension.as_deref().unwrap_or("");

    // --- Icon Selection ---
    let icon = if is_dir {
        if is_expanded {
            FOLDER_OPEN
        } else {
            FOLDER
        }
    } else {
        ICONS.get(extension).copied().unwrap_or(FILE)
    };

    // --- Row Layout ---
    ui.horizontal(|ui| {
        // Indentation
        ui.add_space(depth as f32 * ui.spacing().indent);

        // 1. Checkbox
        let initial_check_state = node_state != Check::Unchecked;
        let mut current_check_state = initial_check_state;
        let checkbox_response = ui.add(egui::Checkbox::new(&mut current_check_state, ""));
        if node_state == Check::Partial {
            let rect = checkbox_response.rect;
            let smaller_rect = rect.shrink(rect.width() * 0.3);
            ui.painter().rect_filled(
                smaller_rect,
                CornerRadius::ZERO,
                ui.visuals().strong_text_color(),
            );
        }
        if checkbox_response.clicked() {
            app.queue_action(AppAction::ToggleCheckState(node_id));
        }

        // 2. Expand/Collapse Toggle
        if is_dir {
            let toggle_icon = if is_expanded { CARET_DOWN } else { CARET_RIGHT };
            if ui
                .selectable_label(false, RichText::new(toggle_icon).size(14.0))
                .clicked()
            {
                app.queue_action(AppAction::ToggleExpandState(node_id));
            }
        } else {
            ui.label(RichText::new(DOT_OUTLINE).size(14.0).weak());
        }

        // 3. Icon and Name Label
        let label_text = format!("{} {}", icon, name);
        let is_selected = app.selected_node_id == Some(node_id);
        let display_text = if !app.search_text.is_empty()
            && name
                .to_lowercase()
                .contains(&app.search_text.to_lowercase())
        {
            RichText::new(label_text)
                .strong()
                .color(ui.visuals().hyperlink_color)
        } else {
            RichText::new(label_text)
        };
        let label_response = ui.selectable_label(is_selected, display_text);

        if label_response.clicked() {
            app.selected_node_id = Some(node_id);
        }
        if label_response.double_clicked() && is_dir {
            app.queue_action(AppAction::ToggleExpandState(node_id));
        }

        // 4. Context Menu
        label_response.context_menu(|ui| {
            let node_id_clone = node_id;
            // Extract node data first to avoid borrow conflicts
            let (node_name, node_path, is_expanded, is_directory) =
                if let Some(ctx_node) = app.nodes.get(node_id) {
                    (
                        ctx_node.name().to_string(),
                        ctx_node.path().display().to_string(),
                        ctx_node.is_expanded,
                        ctx_node.is_dir(),
                    )
                } else {
                    ui.label(RichText::new("Error: Node data unavailable").color(Color32::RED));
                    return;
                };

            ui.label(RichText::new(node_name).strong());
            ui.label(RichText::new(node_path).small().weak());
            ui.separator();

            if ui.button("Toggle Selection").clicked() {
                app.queue_action(AppAction::ToggleCheckState(node_id_clone));
                ui.close_menu();
            }

            if is_directory {
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
            } else if ui.button("Preview File").clicked() {
                app.selected_node_id = Some(node_id_clone);
                app.show_preview_panel = true;
                ui.close_menu();
            }

            ui.separator();
            if ui.button("Open Externally").clicked() {
                app.queue_action(AppAction::OpenNodeExternally(node_id_clone));
                ui.close_menu();
            }
        });
    });
}

/// Helper function to check if a node or any of its descendants match the search text.
fn check_search_match_recursive(app: &CodebaseApp, node_id: FileId, lower_search: &str) -> bool {
    if lower_search.is_empty() {
        return true;
    }

    if let Some(node) = app.nodes.get(node_id) {
        if node.name().to_lowercase().contains(lower_search) {
            return true;
        }
        if node.is_dir() {
            return node
                .children
                .iter()
                .any(|&child_id| check_search_match_recursive(app, child_id, lower_search));
        }
    }
    false
}
