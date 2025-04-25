//! Formats the collected `ReportData` into a Markdown report string.

use super::ReportData;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

// --- Static Mappings and Regex ---

// Map file extensions (lowercase) to Markdown language hints for code fences.
// Add more mappings as needed for common languages/formats.
static LANG_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        // Code
        ("rs", "rust"),
        ("toml", "toml"),
        ("html", "html"),
        ("htm", "html"),
        ("css", "css"),
        ("js", "javascript"),
        ("ts", "typescript"),
        ("jsx", "jsx"),
        ("tsx", "tsx"),
        ("py", "python"),
        ("java", "java"),
        ("c", "c"),
        ("h", "c"),
        ("cpp", "cpp"),
        ("hpp", "cpp"),
        ("cs", "csharp"),
        ("go", "go"),
        ("rb", "ruby"),
        ("php", "php"),
        ("swift", "swift"),
        ("kt", "kotlin"),
        ("sql", "sql"),
        ("sh", "bash"),
        ("bash", "bash"),
        ("ps1", "powershell"),
        ("rb", "ruby"),
        // Markup & Data
        ("md", "markdown"),
        ("json", "json"),
        ("yaml", "yaml"),
        ("yml", "yaml"),
        ("xml", "xml"),
        // Config & Other
        ("lock", "toml"), // e.g., Cargo.lock
        ("gitignore", "gitignore"),
        ("dockerfile", "dockerfile"),
        ("conf", "plaintext"),
        ("cfg", "plaintext"),
        ("ini", "ini"),
        ("log", "log"),
        ("diff", "diff"),
        ("patch", "diff"),
        ("txt", "text"),
        // Add more...
    ])
});

// Regex to replace three or more consecutive newlines with exactly two.
// Helps enforce Markdown linting rule MD012 (no multiple consecutive blank lines).
static MULTIPLE_BLANKS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n{3,}").unwrap());

/// Generates a Markdown report string from the provided `ReportData`.
///
/// Applies formatting rules for headers, lists, code blocks, etc., aiming
/// for compatibility with common Markdown renderers and linters.
pub fn format_markdown(data: &ReportData) -> String {
    let mut md = String::with_capacity(estimate_markdown_capacity(data)); // Pre-allocate buffer

    // --- Report Header ---
    // MD041: First line should be a top-level heading
    md.push_str(&format!("# {} - Codebase Overview\n\n", data.project_name));
    md.push_str(&format!("Generated on: {}\n", data.timestamp));
    // MD047: Files should end with a single newline character (handled at the end)
    // Use inline code for the path to handle special characters
    md.push_str(&format!("Root Path: `{}`\n\n", data.root_path));
    md.push_str("---\n\n"); // Thematic break

    // --- Statistics Section ---
    if let Some(stats) = &data.stats {
        // MD025: Multiple top-level headings (allowed if logical sections)
        md.push_str("## Project Statistics (Full Scan)\n\n");
        // MD032: Lists should be surrounded by blank lines (handled by `\n\n` around sections)
        // Use dashes for unordered lists (consistent style)
        md.push_str(&format!("- **Total Files:** {}\n", stats.total_files));
        md.push_str(&format!("- **Total Dirs:** {}\n", stats.total_dirs));
        md.push_str(&format!("- **Total Size:** {}\n", stats.total_size_human()));

        // File Types
        if !stats.file_types.is_empty() {
            md.push_str("\n**File Types (Count):**\n\n");
            let mut sorted_types: Vec<_> = stats.file_types.iter().collect();
            sorted_types.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending
            for (ext, count) in sorted_types.iter().take(20) {
                // Show top 20
                md.push_str(&format!("- `{}`: {}\n", ext, count)); // Use inline code for extension
            }
            if sorted_types.len() > 20 {
                md.push_str("- ... and more\n");
            }
        }

        // Largest Files
        if !stats.largest_files.is_empty() {
            md.push_str("\n**Largest Files:**\n\n");
            for file_info in &stats.largest_files {
                // Use inline code for path
                md.push_str(&format!(
                    "- `{}` ({})\n",
                    file_info.path, file_info.human_size
                ));
            }
        }

        // Scan Errors
        if !stats.errors.is_empty() {
            md.push_str("\n**Scan Errors:**\n\n");
            // MD031: Fenced code blocks should be surrounded by blank lines
            md.push_str("```log\n"); // Use 'log' hint for errors
            for error in stats.errors.iter().take(10) {
                // Show top 10 errors
                // Assume errors don't contain Markdown formatting characters that need escaping
                md.push_str(&format!("- {}\n", error));
            }
            if stats.errors.len() > 10 {
                md.push_str("- ... and more errors truncated\n");
            }
            md.push_str("```\n\n"); // Ensure blank line after fence
        }
        md.push_str("---\n\n");
    }

    // --- Full Tree Structure Section ---
    md.push_str("## Full Directory Structure\n\n");
    md.push_str("```text\n"); // Use 'text' hint for generic tree structure
    md.push_str(&data.full_tree_structure);
    md.push_str("\n```\n\n"); // Ensure blank line after fence

    md.push_str("---\n\n");

    // --- Selected Tree Structure Section ---
    md.push_str("## Selected Directory Structure\n\n");
    md.push_str("```text\n");
    md.push_str(if data.selected_tree_structure.trim().is_empty() {
        "(No items selected)"
    } else {
        &data.selected_tree_structure
    });
    md.push_str("\n```\n\n"); // Ensure blank line after fence

    md.push_str("---\n\n");

    // --- Selected File Contents Section ---
    md.push_str("## Selected File Contents\n\n");
    if !data.file_details.is_empty() {
        for detail in &data.file_details {
            // MD022: Headings should be surrounded by blank lines (handled by `\n\n`)
            // Use level 3 heading with inline code for the file path
            md.push_str(&format!("### `{}`\n\n", detail.relative_path));
            // Use italics for metadata, not a heading
            md.push_str(&format!(
                "*Size: {} | Modified: {}*\n\n",
                detail.size, detail.modified
            ));

            // Determine language hint for fenced code block
            let ext = Path::new(&detail.relative_path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let lang_hint = LANG_MAP.get(ext).copied().unwrap_or("text"); // Default to 'text'

            md.push_str(&format!("```{}\n", lang_hint));
            match &detail.content {
                // Trim whitespace from content before adding to avoid extra blank lines inside fence
                Ok(content) => md.push_str(content.trim()),
                Err(reason) => md.push_str(reason),
            }
            md.push_str("\n```\n\n"); // Ensure blank line after fence
        }
    } else {
        // Check if stats were included to determine if content was just disabled or truly empty
        let message = if data.stats.is_some() {
            "_(File content inclusion disabled or no files selected)_\n\n"
        } else {
            "_(No files selected)_\n\n"
        };
        md.push_str(message);
    }

    // --- Post-processing ---
    // MD012: No multiple consecutive blank lines
    let cleaned_md = MULTIPLE_BLANKS_RE.replace_all(&md, "\n\n");

    // MD047: Ensure single trailing newline
    let mut final_output = cleaned_md.trim_end().to_string();
    final_output.push('\n');

    final_output
}

/// Estimates the required capacity for the Markdown string buffer.
fn estimate_markdown_capacity(data: &ReportData) -> usize {
    // Similar estimation logic as HTML, potentially slightly smaller due to less markup
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
