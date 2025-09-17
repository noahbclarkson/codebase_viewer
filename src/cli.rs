use std::{env, fs, path::PathBuf};

use anyhow::{anyhow, bail, Context};
use arboard::Clipboard;
use clap::{Args, Parser, Subcommand, ValueEnum};
use crossbeam_channel::unbounded;

use crate::{
    app::state::CodebaseApp,
    config::AppConfig,
    fs::scanner,
    model::Check,
    report::{self, ReportFormat, ReportOptions},
    selection,
};

#[derive(Parser, Debug)]
#[command(author, version, about = "Scan, explore, and document codebases", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate a report and copy it to the clipboard.
    Copy {
        /// The root directory to scan. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Select all files in the codebase.
        #[arg(long, short, default_value_t = false)]
        all: bool,

        /// Path to a JSON file to load a selection from.
        #[arg(long, short)]
        selection: Option<PathBuf>,

        #[command(flatten)]
        report_opts: ReportCliOptions,
    },
    /// Generate a report and save it to a file.
    Generate {
        /// The root directory to scan. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// The output file path for the report.
        #[arg(long, short)]
        output: PathBuf,

        /// Select all files in the codebase.
        #[arg(long, short, default_value_t = false)]
        all: bool,

        /// Path to a JSON file to load a selection from.
        #[arg(long, short)]
        selection: Option<PathBuf>,

        #[command(flatten)]
        report_opts: ReportCliOptions,
    },
    /// Query the codebase using the Gemini AI model.
    Query {
        /// The query/prompt to send to the AI.
        prompt: String,

        /// The root directory to scan. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Select all files in the codebase.
        #[arg(long, short, default_value_t = false)]
        all: bool,

        /// Path to a JSON file to load a selection from.
        #[arg(long, short)]
        selection: Option<PathBuf>,

        #[command(flatten)]
        report_opts: ReportCliOptions,
    },
}

#[derive(Debug, Clone, Args)]
pub struct ReportCliOptions {
    /// The format for the report context.
    #[arg(long, value_enum, default_value_t = CliReportFormat::Markdown)]
    pub format: CliReportFormat,

    /// Exclude the statistics section from the report context.
    #[arg(long, default_value_t = false)]
    pub no_stats: bool,

    /// Exclude file contents from the report context.
    #[arg(long, default_value_t = false)]
    pub no_contents: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum CliReportFormat {
    Markdown,
    Html,
    Text,
}

pub async fn run_cli_command(command: Commands) -> anyhow::Result<()> {
    match command {
        Commands::Copy {
            path,
            all,
            selection,
            report_opts,
        } => {
            let config = AppConfig::load();
            let report_content =
                generate_report_headless(config, path, all, selection, &report_opts)?;
            let mut clipboard = Clipboard::new().context("Failed to access system clipboard")?;
            clipboard
                .set_text(report_content)
                .context("Failed to copy report to clipboard")?;
            println!("Report copied to clipboard.");
        }
        Commands::Generate {
            path,
            output,
            all,
            selection,
            report_opts,
        } => {
            let config = AppConfig::load();
            let report_content =
                generate_report_headless(config, path, all, selection, &report_opts)?;
            fs::write(&output, report_content)
                .with_context(|| format!("Failed to write report to '{}'", output.display()))?;
            println!("Report saved to {}", output.display());
        }
        Commands::Query {
            prompt,
            path,
            all,
            selection,
            report_opts,
        } => {
            let trimmed_prompt = prompt.trim().to_owned();
            if trimmed_prompt.is_empty() {
                bail!("Query prompt cannot be empty");
            }
            let config = AppConfig::load();
            let api_key = env::var("GEMINI_API_KEY")
                .ok()
                .or_else(|| config.gemini_api_key.clone())
                .context("GEMINI_API_KEY not found in environment or configuration")?;
            let report_content =
                generate_report_headless(config, path, all, selection, &report_opts)?;
            println!("Sending query to Gemini...");
            let response = crate::llm::gemini_service::query_codebase(
                &api_key,
                "gemini-2.5-pro",
                report_content,
                trimmed_prompt,
                2.0,
            )
            .await?;
            println!("\n--- AI Response ---\n");
            println!("{}", response);
        }
    }

    Ok(())
}

fn generate_report_headless(
    config: AppConfig,
    path: PathBuf,
    select_all: bool,
    selection_file: Option<PathBuf>,
    report_opts: &ReportCliOptions,
) -> anyhow::Result<String> {
    let mut app_state = CodebaseApp::headless_from_config(config);
    let absolute_path = path
        .canonicalize()
        .with_context(|| format!("Failed to resolve provided path '{}'", path.display()))?;
    let show_hidden = app_state.config.show_hidden_files;
    let respect_cbvignore = app_state.config.respect_cbvignore;

    let (sender, receiver) = unbounded();
    let (handle, _cancel_signal) = scanner::scan(
        absolute_path.clone(),
        show_hidden,
        respect_cbvignore,
        sender,
    );

    app_state.root_path = Some(absolute_path.clone());

    while let Ok(message) = receiver.recv() {
        if app_state.process_scan_message(message) {
            break;
        }
    }

    handle
        .join()
        .map_err(|_| anyhow!("Scanner thread panicked"))?;

    if let Some(root_id) = app_state.root_id {
        if select_all {
            app_state.set_node_state_recursive(root_id, Check::Checked);
        } else if let Some(selection_path) = selection_file {
            let selection_applied = selection::load_selection_from_file(
                &mut app_state.nodes,
                app_state.root_id,
                &selection_path,
            )?;
            app_state.recalculate_all_parent_states(root_id);
            println!("Selection loaded for root: {}", selection_applied);
        }
    } else {
        bail!(
            "Scan completed but no root node was created. Ensure the directory contains readable files."
        );
    }

    let report_options = ReportOptions {
        format: match report_opts.format {
            CliReportFormat::Markdown => ReportFormat::Markdown,
            CliReportFormat::Html => ReportFormat::Html,
            CliReportFormat::Text => ReportFormat::Text,
        },
        include_stats: !report_opts.no_stats,
        include_contents: !report_opts.no_contents,
    };

    app_state.last_report_options = report_options.clone();

    report::generate_report(&app_state, &report_options)
}
