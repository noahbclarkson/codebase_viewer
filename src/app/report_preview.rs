use std::{
    collections::hash_map::DefaultHasher,
    fmt::Write,
    hash::{Hash, Hasher},
    thread,
};

use crate::{
    llm::token_counter,
    model::Check,
    report::{self, ReportOptions},
    task::TaskMessage,
};

use super::state::{CodebaseApp, PreviewExclusion, ReportPreviewState, TokenStatus};

impl CodebaseApp {
    pub(crate) fn mark_report_preview_dirty(&mut self) {
        self.report_preview_dirty = true;
        self.report_preview_state = None;
    }

    pub(crate) fn resolve_gemini_api_key(&self) -> Option<String> {
        std::env::var("GOOGLE_API_KEY")
            .ok()
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
            .or_else(|| self.config.gemini_api_key.clone())
    }

    pub(crate) fn ensure_report_preview(
        &mut self,
        options: &ReportOptions,
    ) -> Option<&ReportPreviewState> {
        let fingerprint = self.selection_fingerprint();
        let needs_rebuild = self.report_preview_dirty
            || self
                .report_preview_state
                .as_ref()
                .map_or(true, |state| state.selection_fingerprint != fingerprint)
            || self
                .report_preview_state
                .as_ref()
                .is_some_and(|state| state.last_options != *options);

        if needs_rebuild {
            let new_state = self.rebuild_report_preview(options, fingerprint);
            self.report_preview_state = Some(new_state);
            self.report_preview_dirty = false;
        }

        let preview_text_to_count = if let Some(state) = self.report_preview_state.as_mut() {
            if matches!(state.token_status, TokenStatus::Idle) {
                if state.preview_text.trim().is_empty() {
                    state.token_status = TokenStatus::NotApplicable;
                    None
                } else {
                    Some(state.preview_text.clone())
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(text) = preview_text_to_count {
            match self.start_token_count_job(&text) {
                Ok(job_id) => {
                    if let Some(state) = self.report_preview_state.as_mut() {
                        state.pending_job_id = Some(job_id);
                        state.token_status = TokenStatus::Loading;
                    }
                }
                Err(message) => {
                    if let Some(state) = self.report_preview_state.as_mut() {
                        state.pending_job_id = None;
                        state.token_status = TokenStatus::Error(message);
                    }
                }
            }
        }

        self.report_preview_state.as_ref()
    }

    fn rebuild_report_preview(
        &self,
        options: &ReportOptions,
        fingerprint: u64,
    ) -> ReportPreviewState {
        let selected_files = self
            .nodes
            .iter()
            .filter(|node| !node.is_dir() && node.state == Check::Checked)
            .count();

        let mut preview_text = String::new();
        let mut excluded_files = Vec::new();
        let mut included_files = 0usize;

        if options.include_contents {
            let details = report::preview_file_details(self, options);
            for detail in details {
                match detail.content {
                    Ok(content) => {
                        included_files += 1;
                        if !preview_text.is_empty() {
                            preview_text.push_str("\n\n");
                        }
                        writeln!(&mut preview_text, "## {}", detail.relative_path).ok();
                        preview_text.push('\n');
                        preview_text.push_str(&content);
                        if !content.ends_with('\n') {
                            preview_text.push('\n');
                        }
                    }
                    Err(reason) => {
                        excluded_files.push(PreviewExclusion {
                            path: detail.relative_path,
                            reason,
                        });
                    }
                }
            }
        }

        let total_characters = preview_text.chars().count();
        let preview_lines = Self::build_preview_line_ranges(&preview_text);
        let token_status = if options.include_contents && included_files > 0 {
            TokenStatus::Idle
        } else {
            TokenStatus::NotApplicable
        };

        ReportPreviewState {
            last_options: options.clone(),
            selection_fingerprint: fingerprint,
            selected_files,
            included_files,
            total_characters,
            preview_text,
            preview_lines,
            excluded_files,
            token_status,
            pending_job_id: None,
        }
    }

    fn selection_fingerprint(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (idx, node) in self.nodes.iter().enumerate() {
            let state_value = match node.state {
                Check::Unchecked => 0u8,
                Check::Checked => 1u8,
                Check::Partial => 2u8,
            };
            (idx as u64, state_value).hash(&mut hasher);
        }
        hasher.finish()
    }

    fn build_preview_line_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return Vec::new();
        }

        let mut ranges = Vec::new();
        let mut start = 0usize;
        for (idx, &byte) in bytes.iter().enumerate() {
            if byte == b'\n' {
                let mut end = idx;
                if end > start && bytes[end - 1] == b'\r' {
                    end -= 1;
                }
                ranges.push(start..end);
                start = idx + 1;
            }
        }
        if start < bytes.len() {
            let mut end = bytes.len();
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            ranges.push(start..end);
        }
        ranges
    }

    fn start_token_count_job(&mut self, preview_text: &str) -> Result<u64, String> {
        let task_sender = self
            .task_sender
            .clone()
            .ok_or_else(|| "Token counting unavailable: task channel missing.".to_string())?;

        let job_id = self.next_token_job_id;
        self.next_token_job_id = self.next_token_job_id.wrapping_add(1);

        let text = preview_text.to_owned();
        let handle = thread::Builder::new()
            .name(format!("token_count_{job_id}"))
            .spawn(move || {
                let result = token_counter::count_tokens(&text);
                let _ = task_sender.send(TaskMessage::TokenCountFinished { job_id, result });
            });

        handle
            .map(|_| job_id)
            .map_err(|err| format!("Failed to spawn token count task: {err}"))
    }
}
