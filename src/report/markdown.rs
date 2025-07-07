//! Formats the collected `ReportData` into a Markdown report string.

use super::ReportData;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

static LANG_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        ("rs", "rust"),
        ("toml", "toml"),
        ("md", "markdown"),
        ("html", "html"),
        ("css", "css"),
        ("js", "javascript"),
        ("ts", "typescript"),
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
        ("json", "json"),
        ("yaml", "yaml"),
        ("yml", "yaml"),
        ("xml", "xml"),
        ("lock", "toml"),
        ("gitignore", "gitignore"),
        ("dockerfile", "dockerfile"),
        ("conf", "plaintext"),
        ("cfg", "plaintext"),
        ("ini", "ini"),
        ("log", "log"),
        ("diff", "diff"),
        ("patch", "diff"),
        ("txt", "text"),
    ])
});

static MULTIPLE_BLANKS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n{3,}").unwrap());

/// Generates a Markdown report string from the provided `ReportData`.
pub fn format_markdown(data: &ReportData) -> String {
    let mut md = String::with_capacity(estimate_markdown_capacity(data));

    md.push_str(&format!("# {} - Codebase Overview\n\n", data.project_name));
    md.push_str(&format!("Generated on: {}\n", data.timestamp));
    md.push_str(&format!("Root Path: `{}`\n\n", data.root_path));
    md.push_str("---\n\n");

    if let Some(stats) = &data.stats {
        md.push_str("## Project Statistics (Full Scan)\n\n");
        md.push_str(&format!("- **Total Files:** {}\n", stats.total_files));
        md.push_str(&format!("- **Total Dirs:** {}\n", stats.total_dirs));
        md.push_str(&format!("- **Total Size:** {}\n", stats.total_size_human()));

        // MODIFIED: Add Language Statistics Table
        if !stats.language_stats.is_empty() {
            md.push_str("\n**Language Statistics:**\n\n");
            md.push_str("| Language | Files | Lines | Code | Comments | Blanks |\n");
            md.push_str("|---|---:|---:|---:|---:|---:|\n"); // Align columns
            let mut sorted_langs: Vec<_> = stats.language_stats.iter().collect();
            sorted_langs.sort_by(|a, b| b.1.code.cmp(&a.1.code));
            for (lang_type, lang) in sorted_langs {
                md.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
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
            md.push_str("\n**File Types (Count):**\n\n");
            let mut sorted_types: Vec<_> = stats.file_types.iter().collect();
            sorted_types.sort_by(|a, b| b.1.cmp(a.1));
            for (ext, count) in sorted_types.iter().take(20) {
                md.push_str(&format!("- `{}`: {}\n", ext, count));
            }
            if sorted_types.len() > 20 {
                md.push_str("- ... and more\n");
            }
        }

        if !stats.largest_files.is_empty() {
            md.push_str("\n**Largest Files:**\n\n");
            for file_info in &stats.largest_files {
                md.push_str(&format!(
                    "- `{}` ({})\n",
                    file_info.path, file_info.human_size
                ));
            }
        }

        if !stats.errors.is_empty() {
            md.push_str("\n**Scan Errors:**\n\n");
            md.push_str("```log\n");
            for error in stats.errors.iter().take(10) {
                md.push_str(&format!("- {}\n", error));
            }
            if stats.errors.len() > 10 {
                md.push_str("- ... and more errors truncated\n");
            }
            md.push_str("```\n\n");
        }
        md.push_str("---\n\n");
    }

    md.push_str("## Full Directory Structure\n\n");
    md.push_str("```text\n");
    md.push_str(&data.full_tree_structure);
    md.push_str("\n```\n\n");

    md.push_str("---\n\n");

    md.push_str("## Selected Directory Structure\n\n");
    md.push_str("```text\n");
    md.push_str(if data.selected_tree_structure.trim().is_empty() {
        "(No items selected)"
    } else {
        &data.selected_tree_structure
    });
    md.push_str("\n```\n\n");

    md.push_str("---\n\n");

    md.push_str("## Selected File Contents\n\n");
    if !data.file_details.is_empty() {
        for detail in &data.file_details {
            md.push_str(&format!("### `{}`\n\n", detail.relative_path));
            md.push_str(&format!(
                "*Size: {} | Modified: {}*\n\n",
                detail.size, detail.modified
            ));
            let ext = Path::new(&detail.relative_path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let lang_hint = LANG_MAP.get(ext).copied().unwrap_or("text");
            md.push_str(&format!("```{}\n", lang_hint));
            match &detail.content {
                Ok(content) => md.push_str(content.trim()),
                Err(reason) => md.push_str(reason),
            }
            md.push_str("\n```\n\n");
        }
    } else {
        let message = if data.stats.is_some() {
            "_(File content inclusion disabled or no files selected)_\n\n"
        } else {
            "_(No files selected)_\n\n"
        };
        md.push_str(message);
    }

    let cleaned_md = MULTIPLE_BLANKS_RE.replace_all(&md, "\n\n");
    let mut final_output = cleaned_md.trim_end().to_string();
    final_output.push('\n');
    final_output
}

fn estimate_markdown_capacity(data: &ReportData) -> usize {
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
