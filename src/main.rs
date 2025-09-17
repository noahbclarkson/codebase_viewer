//! # Codebase Viewer RS
//!
//! This is the main entry point for the Codebase Viewer application.
//! It sets up logging, initializes the egui framework (eframe), loads assets,
//! and either runs the main application loop (`CodebaseApp`) or executes CLI commands.

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::app::state::CodebaseApp;

pub mod app;
mod cli;
pub mod config;
pub mod external;
pub mod fs;
pub mod llm;
pub mod model;
pub mod preview;
pub mod report;
pub mod selection;
pub mod task;
pub mod ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli_args = cli::Cli::parse();

    if let Some(command) = cli_args.command {
        cli::run_cli_command(command).await?
    } else {
        run_gui().map_err(|err| anyhow::anyhow!(err.to_string()))?;
    }

    Ok(())
}

fn run_gui() -> eframe::Result<()> {
    log::info!("Starting Codebase Viewer v{}", env!("CARGO_PKG_VERSION"));

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("Codebase Viewer"),
        persist_window: true,
        ..Default::default()
    };

    log::info!("Loading syntax highlighting assets...");
    let (syntax_set_ref, theme_set_ref) = preview::load_syntax_highlighting_assets();
    log::info!("Syntax highlighting assets loaded.");

    eframe::run_native(
        "codebase_viewer",
        native_options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            egui_material_icons::initialize(&cc.egui_ctx);
            let mut font_definitions = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut font_definitions, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(font_definitions);

            log::info!("Creating App instance...");
            let app = CodebaseApp::new(cc, syntax_set_ref, theme_set_ref);
            log::info!("App instance created, starting UI loop.");

            Ok(Box::new(app))
        }),
    )
}
