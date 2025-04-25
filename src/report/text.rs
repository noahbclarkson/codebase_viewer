//! Formats the collected `ReportData` into a plain text report string.

use super::ReportData;

/// Generates a plain text report string from the provided `ReportData`.
///
/// Uses simple separators for readability in text-based environments.
pub fn format_text(data: &ReportData) -> String {
    let mut txt = String::with_capacity(estimate_text_capacity(data)); // Pre-allocate buffer
    let sep = "=".repeat(70);
    let sub_sep = "-".repeat(70);

    // --- Report Header ---
    txt.push_str(&format!("{}\n", sep));
    txt.push_str(&format!(
        "{} - CODEBASE OVERVIEW\n",
        data.project_name.to_uppercase()
    ));
    txt.push_str(&format!("{}\n", sep));
    txt.push_str(&format!("Generated on: {}\n", data.timestamp));
    txt.push_str(&format!("Root Path:    {}\n", data.root_path));
    txt.push_str(&format!("{}\n\n", sep));

    // --- Statistics Section ---
    if let Some(stats) = &data.stats {
        txt.push_str("PROJECT STATISTICS (FULL SCAN)\n");
        txt.push_str(&format!("{}\n", sub_sep));
        txt.push_str(&format!("Total Files:    {}\n", stats.total_files));
        txt.push_str(&format!("Total Dirs:     {}\n", stats.total_dirs));
        txt.push_str(&format!("Total Size:     {}\n", stats.total_size_human()));

        // File Types
        if !stats.file_types.is_empty() {
            txt.push_str("\nFile Types (Count):\n");
            let mut sorted_types: Vec<_> = stats.file_types.iter().collect();
            sorted_types.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
            for (ext, count) in sorted_types.iter().take(20) {
                // Show top 20
                // Basic alignment for readability
                txt.push_str(&format!("  {:<15}: {}\n", ext, count));
            }
            if sorted_types.len() > 20 {
                txt.push_str("  ... and more\n");
            }
        }

        // Largest Files
        if !stats.largest_files.is_empty() {
            txt.push_str("\nLargest Files:\n");
            for file_info in &stats.largest_files {
                txt.push_str(&format!(
                    "  {} ({})\n",
                    file_info.path, file_info.human_size
                ));
            }
        }

        // Scan Errors
        if !stats.errors.is_empty() {
            txt.push_str("\nScan Errors:\n");
            for error in stats.errors.iter().take(10) {
                // Show top 10 errors
                txt.push_str(&format!("- {}\n", error));
            }
            if stats.errors.len() > 10 {
                txt.push_str("- ... and more errors truncated\n");
            }
        }
        txt.push_str(&format!("{}\n\n", sub_sep));
    }

    // --- Full Tree Structure Section ---
    txt.push_str("FULL DIRECTORY STRUCTURE\n");
    txt.push_str(&format!("{}\n", sub_sep));
    txt.push_str(&data.full_tree_structure);
    txt.push_str(&format!("\n{}\n\n", sub_sep)); // Add newline before separator

    // --- Selected Tree Structure Section ---
    txt.push_str("SELECTED DIRECTORY STRUCTURE\n");
    txt.push_str(&format!("{}\n", sub_sep));
    txt.push_str(if data.selected_tree_structure.trim().is_empty() {
        "(No items selected)\n"
    } else {
        &data.selected_tree_structure
    });
    txt.push_str(&format!("\n{}\n\n", sub_sep)); // Add newline before separator

    // --- Selected File Contents Section ---
    txt.push_str("SELECTED FILE CONTENTS\n");
    txt.push_str(&format!("{}\n", sep)); // Use main separator for this section
    if !data.file_details.is_empty() {
        for detail in &data.file_details {
            // File header
            txt.push_str(&format!("\n--- File: {} ---\n", detail.relative_path));
            txt.push_str(&format!(
                "(Size: {} | Modified: {})\n",
                detail.size, detail.modified
            ));
            txt.push_str(&format!("{}\n", sub_sep)); // Sub-separator before content

            // File content or reason for exclusion
            match &detail.content {
                Ok(content) => {
                    // Trim trailing whitespace/newlines from content itself before printing
                    txt.push_str(content.trim_end());
                }
                Err(reason) => txt.push_str(reason),
            }
            // Add exactly two newlines after content block for separation before next file
            txt.push_str("\n\n");
        }
        // Remove the last two extra newlines added by the loop
        txt.truncate(txt.trim_end().len());
        txt.push('\n'); // Ensure one trailing newline for the section
        txt.push_str(&format!("{}\n", sep)); // End separator for the section
    } else {
        // Check if stats were included to determine if content was just disabled or truly empty
        let message = if data.stats.is_some() {
            "(File content inclusion disabled or no files selected)\n"
        } else {
            "(No files selected)\n"
        };
        txt.push_str(message);
        txt.push_str(&format!("{}\n", sep));
    }

    // Ensure final string ends with a single newline
    txt.trim_end().to_string() + "\n"
}

/// Estimates the required capacity for the text report string buffer.
fn estimate_text_capacity(data: &ReportData) -> usize {
    // Similar estimation logic as Markdown/HTML
    let base_size = 1024;
    let tree_size = data.full_tree_structure.len() + data.selected_tree_structure.len();
    let stats_size = if data.stats.is_some() { 512 } else { 0 };
    let file_meta_size = data.file_details.len() * 150;
    let file_content_size: usize = data
        .file_details
        .iter()
        .map(|d| d.content.as_ref().map_or(50, |s| s.len()))
        .sum();

    base_size + tree_size + stats_size + file_meta_size + file_content_size
}
