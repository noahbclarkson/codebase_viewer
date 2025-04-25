//! # Codebase Viewer RS
//!
//! This is the main entry point for the Codebase Viewer application.
//! It sets up logging, initializes the egui framework (eframe), loads assets,
//! and runs the main application loop (`CodebaseApp`).

use crate::app::CodebaseApp;
use tracing_subscriber::EnvFilter;

// Make modules public within the crate for app.rs and others to use them easily
pub mod app;
pub mod config;
pub mod external;
pub mod fs;
pub mod model;
pub mod preview;
pub mod report;
pub mod selection;
pub mod task;
pub mod ui;

/// The main entry point of the application.
/// Initializes logging, sets up eframe native options, loads assets,
/// and starts the eframe run loop with the `CodebaseApp`.
fn main() -> eframe::Result<()> {
    // Initialize tracing subscriber for logging
    // Reads log level from RUST_LOG environment variable (e.g., RUST_LOG=info)
    // Defaults to a reasonable level if RUST_LOG is not set.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    log::info!("Starting Codebase Viewer v{}", env!("CARGO_PKG_VERSION"));

    // Configure native window options for eframe
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0]) // Default window size
            .with_min_inner_size([600.0, 400.0]) // Minimum window size
            .with_title("Codebase Viewer"), // Window title
        persist_window: true, // Remember window position and size
        ..Default::default()
    };

    // Load syntax highlighting assets once at startup.
    // These are static references valid for the lifetime of the application.
    log::info!("Loading syntax highlighting assets...");
    let (syntax_set_ref, theme_set_ref) = preview::load_syntax_highlighting_assets();
    log::info!("Syntax highlighting assets loaded.");

    // Run the eframe application
    eframe::run_native(
        "codebase_viewer_rs", // Unique ID for app state persistence
        native_options,
        // Closure to create the App instance
        Box::new(move |cc| {
            // Install egui image loaders for supported formats (png, jpg, gif, etc.)
            egui_extras::install_image_loaders(&cc.egui_ctx);

            // Initialize icon fonts
            egui_material_icons::initialize(&cc.egui_ctx);
            let mut font_definitions = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut font_definitions, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(font_definitions);

            // Create the main application state
            log::info!("Creating App instance...");
            let app = CodebaseApp::new(cc, syntax_set_ref, theme_set_ref);
            log::info!("App instance created, starting UI loop.");

            // Return the boxed app instance
            Ok(Box::new(app))
        }),
    ) // The `?` operator propagates any error from eframe::run_native
}
