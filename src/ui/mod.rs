//! # UI Module
//!
//! This module contains all the components responsible for drawing the
//! application's user interface using `egui`. It includes panels, dialogs,
//! menus, and status bars.

pub mod dialogs;
pub mod menu_bar;
pub mod preview_panel;
pub mod status_bar;
pub mod tree_panel;

// Re-export the main drawing functions for each UI component
// This allows `app.rs` to call `ui::draw_menu_bar(...)` etc.
pub use dialogs::{
    draw_about_window, draw_preferences_window, draw_report_options_window, draw_shortcuts_window,
};
pub use menu_bar::draw_menu_bar;
pub use preview_panel::draw_preview_panel;
pub use status_bar::draw_status_bar;
pub use tree_panel::draw_tree_panel;
