//! Formats the collected `ReportData` into a plain text report string.

use super::ReportData;

/// Generates a plain text report string from the provided `ReportData`.
pub fn format_text(data: &ReportData) -> String {
    let mut txt = String::with_capacity(estimate_text_capacity(data));
    let sep = "=".repeat(70);
    let sub_sep = "-".repeat(70);

    txt.push_str(&format!("{}\n", sep));
    txt.push_str(&format!(
        "{} - CODEBASE OVERVIEW\n",
        data.project_name.to_uppercase()
    ));
    txt.push_str(&format!("{}\n", sep));
    txt.push_str(&format!("Generated on: {}\n", data.timestamp));
    txt.push_str(&format!("Root Path:    {}\n", data.root_path));
    txt.push_str(&format!("{}\n\n", sep));

    if let Some(stats) = &data.stats {
        txt.push_str("PROJECT STATISTICS (FULL SCAN)\n");
        txt.push_str(&format!("{}\n", sub_sep));
        txt.push_str(&format!("Total Files:    {}\n", stats.total_files));
        txt.push_str(&format!("Total Dirs:     {}\n", stats.total_dirs));
        txt.push_str(&format!("Total Size:     {}\n", stats.total_size_human()));

        // MODIFIED: Add Language Statistics
        if !stats.language_stats.is_empty() {
            txt.push_str("\nLanguage Statistics:\n");
            let mut sorted_langs: Vec<_> = stats.language_stats.iter().collect();
            sorted_langs.sort_by(|a, b| b.1.code.cmp(&a.1.code));
            for (lang_type, lang) in sorted_langs {
                txt.push_str(&format!(
                    "  - {:<15} | Files: {:<5} | Lines: {:<7} (Code: {}, Comments: {}, Blanks: {})\n",
                    lang_type,
                    lang.reports.len(),
                    lang.lines(),
                    lang.code,
                    lang.comments,
                    lang.blanks
                ));
            }
        }

        if !stats.file_types.is_empty() {
            txt.push_str("\nFile Types (Count):\n");
            let mut sorted_types: Vec<_> = stats.file_types.iter().collect();
            sorted_types.sort_by(|a, b| b.1.cmp(a.1));
            for (ext, count) in sorted_types.iter().take(20) {
                txt.push_str(&format!("  {:<15}: {}\n", ext, count));
            }
            if sorted_types.len() > 20 {
                txt.push_str("  ... and more\n");
            }
        }

        if !stats.largest_files.is_empty() {
            txt.push_str("\nLargest Files:\n");
            for file_info in &stats.largest_files {
                txt.push_str(&format!(
                    "  {} ({})\n",
                    file_info.path, file_info.human_size
                ));
            }
        }

        if !stats.errors.is_empty() {
            txt.push_str("\nScan Errors:\n");
            for error in stats.errors.iter().take(10) {
                txt.push_str(&format!("- {}\n", error));
            }
            if stats.errors.len() > 10 {
                txt.push_str("- ... and more errors truncated\n");
            }
        }
        txt.push_str(&format!("{}\n\n", sub_sep));
    }

    txt.push_str("FULL DIRECTORY STRUCTURE\n");
    txt.push_str(&format!("{}\n", sub_sep));
    txt.push_str(&data.full_tree_structure);
    txt.push_str(&format!("\n{}\n\n", sub_sep));

    txt.push_str("SELECTED DIRECTORY STRUCTURE\n");
    txt.push_str(&format!("{}\n", sub_sep));
    txt.push_str(if data.selected_tree_structure.trim().is_empty() {
        "(No items selected)\n"
    } else {
        &data.selected_tree_structure
    });
    txt.push_str(&format!("\n{}\n\n", sub_sep));

    txt.push_str("SELECTED FILE CONTENTS\n");
    txt.push_str(&format!("{}\n", sep));
    if !data.file_details.is_empty() {
        for detail in &data.file_details {
            txt.push_str(&format!("\n--- File: {} ---\n", detail.relative_path));
            txt.push_str(&format!(
                "(Size: {} | Modified: {})\n",
                detail.size, detail.modified
            ));
            txt.push_str(&format!("{}\n", sub_sep));
            match &detail.content {
                Ok(content) => txt.push_str(content.trim_end()),
                Err(reason) => txt.push_str(reason),
            }
            txt.push_str("\n\n");
        }
        txt.truncate(txt.trim_end().len());
        txt.push('\n');
        txt.push_str(&format!("{}\n", sep));
    } else {
        let message = if data.stats.is_some() {
            "(File content inclusion disabled or no files selected)\n"
        } else {
            "(No files selected)\n"
        };
        txt.push_str(message);
        txt.push_str(&format!("{}\n", sep));
    }

    txt.trim_end().to_string() + "\n"
}

fn estimate_text_capacity(data: &ReportData) -> usize {
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
