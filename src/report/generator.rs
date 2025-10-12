//! Core logic for report generation: data collection and formatting dispatch.

use super::{FileDetail, ReportData, ReportFormat, ReportOptions};
use crate::{
    app::CodebaseApp,
    model::{self, Check}, // Use model types
    preview,              // Use preview module for reading file content
};
use std::path::Path;

/// Prepends line numbers to a block of text.
fn prepend_line_numbers(content: &str) -> String {
    let line_count = content.lines().count();
    if line_count == 0 {
        return String::new();
    }
    let line_number_width = (line_count as f64).log10().floor() as usize + 1;

    content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            format!(
                "{number:>width$} | {line}\n",
                number = i + 1,
                width = line_number_width
            )
        })
        .collect()
}

/// Generates the full report content as a string based on options.
///
/// This function orchestrates the process:
/// 1. Collects necessary data from the `CodebaseApp` state (`collect_report_data`).
/// 2. Formats the collected data into the desired output string (`format_report_content`).
///
/// This function is suitable for simple, synchronous report generation. For background
/// generation, `collect_report_data` and `format_report_content` should be used separately.
pub fn generate_report(app: &CodebaseApp, options: &ReportOptions) -> anyhow::Result<String> {
    log::info!("Starting report generation with options: {options:?}");
    // 1. Collect data
    let data = collect_report_data(app, options)?;
    log::info!("Report data collected successfully.");
    // 2. Format data
    let report_string = format_report_content(&data, options)?;
    log::info!("Report formatting complete.");
    Ok(report_string)
}

/// Collects all necessary data from the application state for report generation.
///
/// This function gathers information like project name, paths, tree structures,
/// selected file details (potentially reading file content), and scan statistics.
/// It returns an owned `ReportData` struct, suitable for passing to background threads.
///
/// # Arguments
/// * `app` - A reference to the main `CodebaseApp` state.
/// * `options` - The `ReportOptions` specifying what data to include.
///
/// # Returns
/// * `Ok(ReportData)` containing the collected information.
/// * `Err(anyhow::Error)` if essential data (like `root_path`) is missing.
pub fn collect_report_data(
    app: &CodebaseApp,
    options: &ReportOptions,
) -> anyhow::Result<ReportData> {
    log::debug!("Collecting report data...");
    // Ensure a directory is open
    let root_path = app.root_path.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Cannot generate report: No directory is currently open.")
    })?;

    let project_name = root_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown Project".to_string());
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // Generate tree structure strings
    log::debug!("Generating full tree structure string...");
    let full_tree_structure = generate_tree_string(app, false); // Include all nodes
    log::debug!("Generating selected tree structure string...");
    let selected_tree_structure = generate_tree_string(app, true); // Include only selected nodes/ancestors

    // Collect details for selected files (including content if requested)
    let file_details = if options.include_contents {
        log::debug!("Collecting file details (including content)...");
        collect_file_details(app, app.config.max_file_size_preview, options)
    } else {
        log::debug!("Collecting file details (excluding content)...");
        // Collect details but skip content reading
        collect_file_details_metadata_only(app)
    };
    log::debug!(
        "Collected details for {} selected files.",
        file_details.len()
    );

    // Include scan statistics if requested
    let stats = if options.include_stats {
        log::debug!("Including scan statistics.");
        app.scan_stats.clone() // Clone the Option<ScanStats>
    } else {
        log::debug!("Excluding scan statistics.");
        None
    };

    // Construct the owned ReportData struct
    Ok(ReportData {
        project_name,
        timestamp,
        root_path: root_path.display().to_string(),
        full_tree_structure,
        selected_tree_structure,
        file_details,
        stats,
    })
}

/// Formats the collected `ReportData` into the final report string based on `ReportOptions`.
///
/// This function dispatches to the appropriate formatting function (Markdown, HTML, Text).
/// It's designed to be called after `collect_report_data`, potentially in a background thread.
pub fn format_report_content(data: &ReportData, options: &ReportOptions) -> anyhow::Result<String> {
    log::debug!("Formatting report content as {:?}", options.format);
    let report_content = match options.format {
        ReportFormat::Markdown => super::markdown::format_markdown(data),
        ReportFormat::Html => super::html::format_html(data),
        ReportFormat::Text => super::text::format_text(data),
    };
    // Basic validation or post-processing could happen here if needed
    Ok(report_content)
}

// --- Helper Functions ---

/// Generates a text representation of the file tree structure.
///
/// # Arguments
/// * `app` - Reference to the application state.
/// * `selected_only` - If true, only includes nodes that are Checked or Partial,
///   and their ancestors. If false, includes all nodes.
fn generate_tree_string(app: &CodebaseApp, selected_only: bool) -> String {
    let mut output = String::new();
    if let Some(root_id) = app.root_id {
        if let Some(root_node) = app.nodes.get(root_id) {
            // Check if the root itself should be included based on selection status
            if !selected_only || root_node.state != Check::Unchecked {
                output.push_str(root_node.name()); // Add root node name
                output.push('\n');

                // Filter children based on selection status if needed
                let children_to_render: Vec<_> = root_node
                    .children
                    .iter()
                    .filter(|&&child_id| {
                        !selected_only
                            || app
                                .nodes
                                .get(child_id)
                                .is_some_and(|n| n.state != Check::Unchecked)
                    })
                    .cloned()
                    .collect();

                let num_children = children_to_render.len();
                for (i, &child_id) in children_to_render.iter().enumerate() {
                    // Start recursion for children
                    build_tree_string_recursive(
                        app,
                        &mut output,
                        child_id,
                        "",                    // Initial prefix
                        i == num_children - 1, // is_last flag
                        selected_only,
                    );
                }
            } else if selected_only {
                // If root is unchecked and we only want selected, the tree is empty
                return "(Root directory not selected)".to_string();
            }
        }
    } else {
        return "(No directory loaded)".to_string();
    }
    output.trim_end().to_string() // Trim trailing newline
}

/// Recursive helper function for building the tree string with ASCII art connectors.
fn build_tree_string_recursive(
    app: &CodebaseApp,
    output: &mut String,
    node_id: model::FileId,
    prefix: &str,
    is_last: bool, // Is this the last sibling in the current level?
    selected_only: bool,
) {
    let node = match app.nodes.get(node_id) {
        Some(n) => n,
        None => {
            log::error!("generate_tree_string: Invalid node ID {node_id} encountered.");
            return; // Skip invalid nodes
        }
    };

    // Determine the connector based on whether it's the last sibling
    let connector = if is_last { "└── " } else { "├── " };
    let line = format!("{}{}{}\n", prefix, connector, node.name());
    output.push_str(&line);

    // If it's a directory, recurse into its children
    if node.is_dir() {
        // Calculate the prefix for the children's lines
        let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

        // Filter children if only selected nodes are needed
        let children_to_render: Vec<_> = node
            .children
            .iter()
            .filter(|&&child_id| {
                !selected_only
                    || app
                        .nodes
                        .get(child_id)
                        .is_some_and(|n| n.state != Check::Unchecked)
            })
            .cloned()
            .collect();

        let num_children = children_to_render.len();
        for (i, &child_id) in children_to_render.iter().enumerate() {
            build_tree_string_recursive(
                app,
                output,
                child_id,
                &child_prefix,
                i == num_children - 1, // Pass is_last flag to children
                selected_only,
            );
        }
    }
}

/// Collects content and metadata for all *selected* files.
/// Reads file content based on `max_size` limit.
fn collect_file_details(
    app: &CodebaseApp,
    max_size: i64,
    options: &ReportOptions,
) -> Vec<FileDetail> {
    let mut details = Vec::new();
    let root_path = app.root_path.as_deref().unwrap_or_else(|| Path::new(""));

    for node in &app.nodes {
        // Only include files that are explicitly checked
        if !node.is_dir() && node.state == Check::Checked {
            let path = node.path();
            let relative_path = path
                .strip_prefix(root_path)
                .unwrap_or(path) // Fallback to absolute if strip fails
                .display()
                .to_string();

            // Format modification time
            let modified_str = node
                .info
                .modified
                .map(|st| {
                    let datetime: chrono::DateTime<chrono::Local> = st.into();
                    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
                })
                .unwrap_or_else(|| "N/A".to_string());

            // Attempt to read content, handling binary files and size limits
            let content_result = if node.info.is_binary {
                Err("[Binary file content not shown]".to_string())
            } else {
                // Use the preview module's reader which handles size limits and encoding
                preview::read_file_content(path, max_size)
            };

            // Conditionally prepend line numbers
            let final_content = if options.include_line_numbers {
                content_result.map(|text| prepend_line_numbers(&text))
            } else {
                content_result
            };

            details.push(FileDetail {
                relative_path,
                size: node.info.human_size.clone(),
                modified: modified_str,
                content: final_content,
            });
        }
    }
    // Sort details alphabetically by relative path for consistent report output
    details.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    details
}

/// Collects metadata (but not content) for all *selected* files.
/// Used when `ReportOptions::include_contents` is false.
fn collect_file_details_metadata_only(app: &CodebaseApp) -> Vec<FileDetail> {
    let mut details = Vec::new();
    let root_path = app.root_path.as_deref().unwrap_or_else(|| Path::new(""));

    for node in &app.nodes {
        // Only include files that are explicitly checked
        if !node.is_dir() && node.state == Check::Checked {
            let path = node.path();
            let relative_path = path
                .strip_prefix(root_path)
                .unwrap_or(path) // Fallback to absolute if strip fails
                .display()
                .to_string();

            // Format modification time
            let modified_str = node
                .info
                .modified
                .map(|st| {
                    let datetime: chrono::DateTime<chrono::Local> = st.into();
                    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
                })
                .unwrap_or_else(|| "N/A".to_string());

            details.push(FileDetail {
                relative_path,
                size: node.info.human_size.clone(),
                modified: modified_str,
                // Indicate content was explicitly excluded
                content: Err("[File content excluded by report options]".to_string()),
            });
        }
    }
    // Sort details alphabetically by relative path
    details.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    details
}
