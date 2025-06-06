//! Formats the collected `ReportData` into an HTML report string.

use super::ReportData;

/// Generates an HTML report string from the provided `ReportData`.
///
/// Creates a self-contained HTML document with basic styling.
pub fn format_html(data: &ReportData) -> String {
    // Use a String buffer for efficient concatenation
    let mut html = String::with_capacity(estimate_html_capacity(data)); // Pre-allocate buffer

    // --- HTML Header ---
    html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("  <meta charset=\"UTF-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str(&format!(
        "  <title>Codebase Report: {}</title>\n",
        html_escape(&data.project_name)
    ));
    // Basic CSS for styling
    html.push_str("  <style>\n");
    html.push_str("    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, 'Open Sans', 'Helvetica Neue', sans-serif; line-height: 1.5; margin: 20px; color: #333; }\n");
    html.push_str("    h1, h2, h3 { border-bottom: 1px solid #eee; padding-bottom: 0.3em; margin-top: 1.8em; margin-bottom: 0.8em; color: #111; }\n");
    html.push_str("    h1 { font-size: 1.8em; }\n");
    html.push_str("    h2 { font-size: 1.5em; }\n");
    html.push_str("    h3 { font-size: 1.2em; border-bottom-style: dashed; }\n");
    html.push_str("    pre { background-color: #f8f8f8; padding: 1em; border: 1px solid #ddd; border-radius: 4px; white-space: pre-wrap; word-wrap: break-word; font-size: 0.9em; line-height: 1.4; }\n");
    html.push_str("    code { font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, Courier, monospace; background-color: #eee; padding: 0.15em 0.4em; border-radius: 3px; font-size: 0.95em; }\n");
    html.push_str("    pre > code { background-color: transparent; padding: 0; border-radius: 0; font-size: 1em; }\n"); // Reset code style inside pre
    html.push_str("    .file-header { margin-top: 2.5em; border-bottom: 1px solid #eee; padding-bottom: 0.5em; }\n");
    html.push_str("    .file-meta { font-size: 0.9em; color: #555; margin-bottom: 0.5em; }\n");
    html.push_str("    ul { padding-left: 25px; margin-top: 0.5em; }\n");
    html.push_str("    li { margin-bottom: 0.4em; }\n");
    html.push_str("    hr { border: 0; border-top: 1px solid #ccc; margin: 2.5em 0; }\n");
    html.push_str("    .error-text { color: #c00; }\n");
    html.push_str("    .report-header p { margin: 0.3em 0; color: #444; }\n");
    html.push_str("  </style>\n");
    html.push_str("</head>\n<body>\n");

    // --- Report Header ---
    html.push_str("<header class=\"report-header\">\n");
    html.push_str(&format!(
        "  <h1>{} - Codebase Overview</h1>\n",
        html_escape(&data.project_name)
    ));
    html.push_str(&format!(
        "  <p>Generated on: {}</p>\n",
        html_escape(&data.timestamp)
    ));
    html.push_str(&format!(
        "  <p>Root Path: <code>{}</code></p>\n",
        html_escape(&data.root_path)
    ));
    html.push_str("</header>\n");
    html.push_str("<hr>\n");

    // --- Statistics Section ---
    if let Some(stats) = &data.stats {
        html.push_str("<section id=\"statistics\">\n");
        html.push_str("  <h2>Project Statistics (Full Scan)</h2>\n  <ul>\n");
        html.push_str(&format!(
            "    <li><strong>Total Files:</strong> {}</li>\n",
            stats.total_files
        ));
        html.push_str(&format!(
            "    <li><strong>Total Dirs:</strong> {}</li>\n",
            stats.total_dirs
        ));
        html.push_str(&format!(
            "    <li><strong>Total Size:</strong> {}</li>\n",
            html_escape(&stats.total_size_human())
        ));
        html.push_str("  </ul>\n");

        // File Types
        if !stats.file_types.is_empty() {
            html.push_str("  <h3>File Types (Count):</h3>\n  <ul>\n");
            let mut sorted_types: Vec<_> = stats.file_types.iter().collect();
            sorted_types.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
            for (ext, count) in sorted_types.iter().take(20) {
                // Show top 20
                html.push_str(&format!(
                    "    <li><code>{}</code>: {}</li>\n",
                    html_escape(ext),
                    count
                ));
            }
            if sorted_types.len() > 20 {
                html.push_str("    <li>... and more</li>\n");
            }
            html.push_str("  </ul>\n");
        }

        // Largest Files
        if !stats.largest_files.is_empty() {
            html.push_str("  <h3>Largest Files:</h3>\n  <ul>\n");
            for file_info in &stats.largest_files {
                html.push_str(&format!(
                    "    <li><code>{}</code> ({})</li>\n",
                    html_escape(&file_info.path),
                    html_escape(&file_info.human_size)
                ));
            }
            html.push_str("  </ul>\n");
        }

        // Scan Errors
        if !stats.errors.is_empty() {
            html.push_str("  <h3>Scan Errors:</h3>\n  <pre><code class=\"error-text\">");
            for error in stats.errors.iter().take(10) {
                // Show top 10 errors
                html.push_str(&format!("- {}\n", html_escape(error)));
            }
            if stats.errors.len() > 10 {
                html.push_str("- ... and more errors truncated\n");
            }
            html.push_str("</code></pre>\n");
        }
        html.push_str("</section>\n");
        html.push_str("<hr>\n");
    }

    // --- Full Tree Structure Section ---
    html.push_str("<section id=\"full-tree\">\n");
    html.push_str("  <h2>Full Directory Structure</h2>\n  <pre><code>");
    html.push_str(&html_escape(&data.full_tree_structure));
    html.push_str("</code></pre>\n");
    html.push_str("</section>\n");
    html.push_str("<hr>\n");

    // --- Selected Tree Structure Section ---
    html.push_str("<section id=\"selected-tree\">\n");
    html.push_str("  <h2>Selected Directory Structure</h2>\n  <pre><code>");
    let selected_tree_content = if data.selected_tree_structure.trim().is_empty() {
        "(No items selected)".to_string()
    } else {
        html_escape(&data.selected_tree_structure)
    };
    html.push_str(&selected_tree_content);
    html.push_str("</code></pre>\n");
    html.push_str("</section>\n");
    html.push_str("<hr>\n");

    // --- Selected File Contents Section ---
    html.push_str("<section id=\"file-contents\">\n");
    html.push_str("  <h2>Selected File Contents</h2>\n");
    if !data.file_details.is_empty() {
        for detail in &data.file_details {
            html.push_str("  <div class=\"file-header\">\n");
            html.push_str(&format!(
                "    <h3><code>{}</code></h3>\n",
                html_escape(&detail.relative_path)
            ));
            html.push_str(&format!(
                "    <div class=\"file-meta\">Size: {} | Modified: {}</div>\n",
                html_escape(&detail.size),
                html_escape(&detail.modified)
            ));
            html.push_str("  </div>\n"); // End file-header
            html.push_str("  <pre><code>"); // Code block for content
            match &detail.content {
                Ok(content) => html.push_str(&html_escape(content)),
                Err(reason) => html.push_str(&format!(
                    "<span class=\"error-text\">{}</span>",
                    html_escape(reason)
                )),
            }
            html.push_str("</code></pre>\n");
        }
    } else {
        // Check if stats were included to determine if content was just disabled or truly empty
        let message = if data.stats.is_some() {
            "<em>(File content inclusion disabled or no files selected)</em>"
        } else {
            "<em>(No files selected)</em>"
        };
        html.push_str(&format!("  <p>{}</p>\n", message));
    }
    html.push_str("</section>\n");

    // --- HTML Footer ---
    html.push_str("\n</body>\n</html>\n");
    html
}

/// Basic HTML escaping helper function.
/// Replaces characters that have special meaning in HTML.
fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;") // Escape single quotes as well
}

/// Estimates the required capacity for the HTML string buffer.
/// This is a rough estimate to potentially reduce reallocations.
fn estimate_html_capacity(data: &ReportData) -> usize {
    let base_size = 2048; // Base HTML structure, CSS, headers
    let tree_size = data.full_tree_structure.len() + data.selected_tree_structure.len();
    let stats_size = if data.stats.is_some() { 1024 } else { 0 };
    let file_meta_size = data.file_details.len() * 200; // Estimate for path, size, modified per file
    let file_content_size: usize = data
        .file_details
        .iter()
        .map(|d| d.content.as_ref().map_or(50, |s| s.len())) // Estimate content size or error msg size
        .sum();

    base_size + tree_size + stats_size + file_meta_size + file_content_size
}
