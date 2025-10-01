//! Contains miscellaneous helper functions for the `CodebaseApp`.

use super::state::CodebaseApp;
use crate::{
    model::{Check, FileId},
    preview,
};
use egui::{Context, Visuals};

impl CodebaseApp {
    /// Sets the egui visual theme (light/dark) based on the configuration.
    pub fn set_egui_theme(ctx: &Context, theme_name: &str) {
        log::info!("Setting application theme to: {theme_name}");
        match theme_name {
            "dark" => ctx.set_visuals(Visuals::dark()),
            "light" => ctx.set_visuals(Visuals::light()),
            _ => match dark_light::detect() {
                Ok(dark_light::Mode::Dark) => {
                    log::info!("System theme detected: Dark");
                    ctx.set_visuals(Visuals::dark())
                }
                _ => {
                    log::info!("System theme detected: Light (or default)");
                    ctx.set_visuals(Visuals::light())
                }
            },
        }
    }

    /// Initiates loading the preview content for the specified node in a background thread.
    /// The result will be sent back via the `preview_receiver`.
    pub(crate) fn trigger_preview_load(&mut self, node_id: FileId, ctx: &Context) {
        if let Some(node) = self.nodes.get(node_id) {
            if node.is_dir() {
                self.preview_cache = None;
                return;
            }

            if let Some(cache_mutex) = &self.preview_cache {
                // MODIFIED: Use `if let Some(guard) = ...` for Option
                if let Some(cache) = cache_mutex.try_lock() {
                    let current_theme_name = self.get_current_syntax_theme_name();
                    let theme_matches = match &cache.content {
                        preview::PreviewContent::Text(_) => {
                            cache.theme_used.as_deref() == Some(&current_theme_name)
                        }
                        _ => true,
                    };

                    if cache.node_id == node_id && theme_matches {
                        log::trace!("Preview cache hit for node {node_id}");
                        return;
                    }
                }
            }

            log::trace!("Initiating preview load for node {node_id}");
            let path = node.path().to_path_buf();
            let cfg = self.config.clone();
            let ss = self.syntax_set;
            let ts = self.theme_set;
            let tx = self
                .preview_sender
                .as_ref()
                .expect("Preview sender missing")
                .clone();
            let ctx_clone = ctx.clone();

            rayon::spawn_fifo(move || {
                let cache_entry =
                    preview::generate_preview(&cfg, ss, ts, &path, node_id, &ctx_clone);
                if tx.send((node_id, cache_entry)).is_err() {
                    log::warn!("Failed to send preview result: Channel closed.");
                }
            });
        } else {
            log::warn!("Attempted to load preview for invalid node ID: {node_id}");
        }
    }

    /// Recursively sets the check state for a node and all its descendants.
    pub(crate) fn set_node_state_recursive(&mut self, node_id: FileId, state: Check) {
        let mut queue = vec![node_id];
        while let Some(current_id) = queue.pop() {
            if let Some(node) = self.nodes.get_mut(current_id) {
                node.state = state;
                if node.is_dir() {
                    queue.extend(node.children.clone());
                }
            }
        }
    }

    /// Recursively sets the expansion state for a node and all its descendants.
    pub(crate) fn set_node_expansion_recursive(&mut self, node_id: FileId, expand: bool) {
        let mut queue = vec![node_id];
        while let Some(current_id) = queue.pop() {
            if let Some(node) = self.nodes.get_mut(current_id) {
                if node.is_dir() {
                    node.is_expanded = expand;
                    queue.extend(node.children.clone());
                }
            }
        }
    }

    /// Updates the check state of ancestor nodes based on their children's states.
    pub(super) fn update_parent_states(&mut self, start_node_id: FileId) {
        let mut current_id_opt = Some(start_node_id);
        while let Some(current_id) = current_id_opt {
            if let Some(parent_id) = self.find_parent_id(current_id) {
                let new_parent_state = self.calculate_parent_check_state(parent_id);
                if let Some(parent_node) = self.nodes.get_mut(parent_id) {
                    if parent_node.state == new_parent_state {
                        break;
                    }
                    parent_node.state = new_parent_state;
                    current_id_opt = Some(parent_id);
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Recalculates the check state for all nodes in a subtree.
    pub(crate) fn recalculate_all_parent_states(&mut self, node_id: FileId) {
        let children = self
            .nodes
            .get(node_id)
            .map_or(Vec::new(), |n| n.children.clone());
        for child_id in children {
            self.recalculate_all_parent_states(child_id);
        }
        if let Some(node) = self.nodes.get(node_id) {
            if node.is_dir() && !node.children.is_empty() {
                let new_state = self.calculate_parent_check_state(node_id);
                if let Some(node_mut) = self.nodes.get_mut(node_id) {
                    node_mut.state = new_state;
                }
            }
        }
    }

    /// Finds the `FileId` of the parent node containing `child_id`.
    pub(super) fn find_parent_id(&self, child_id: FileId) -> Option<FileId> {
        // Get the child node's data.
        let child_node = self.nodes.get(child_id)?;

        // Get its parent's path.
        let parent_path = child_node.path().parent()?;

        // Look up the parent's path in the map to get its ID.
        self.path_to_id_map.get(parent_path).copied()
    }

    /// Calculates the correct `Check` state for a parent node based on its children's states.
    pub(super) fn calculate_parent_check_state(&self, parent_id: FileId) -> Check {
        if let Some(parent_node) = self.nodes.get(parent_id) {
            if !parent_node.is_dir() || parent_node.children.is_empty() {
                return parent_node.state;
            }
            let mut has_checked = false;
            let mut has_unchecked = false;
            let mut has_partial = false;
            for &child_id in &parent_node.children {
                if let Some(child_node) = self.nodes.get(child_id) {
                    match child_node.state {
                        Check::Checked => has_checked = true,
                        Check::Unchecked => has_unchecked = true,
                        Check::Partial => has_partial = true,
                    }
                }
                if (has_checked || has_partial) && has_unchecked {
                    return Check::Partial;
                }
                if has_partial {
                    return Check::Partial;
                }
            }
            if has_checked && !has_unchecked {
                Check::Checked
            } else if !has_checked && has_unchecked {
                Check::Unchecked
            } else {
                Check::Partial
            }
        } else {
            Check::Unchecked
        }
    }

    /// Counts the total number of files and the number of selected files.
    pub(crate) fn count_files(&self) -> (usize, usize) {
        let mut total = 0;
        let mut selected = 0;
        for node in &self.nodes {
            if !node.is_dir() {
                total += 1;
                if node.state == Check::Checked {
                    selected += 1;
                }
            }
        }
        (total, selected)
    }

    /// Recursively sorts the children of each node.
    pub(super) fn sort_nodes_recursively(&mut self, node_id_opt: Option<FileId>) {
        if let Some(node_id) = node_id_opt {
            if let Some(node) = self.nodes.get_mut(node_id) {
                if node.is_dir() && !node.children.is_empty() {
                    let mut children = std::mem::take(&mut node.children);
                    children.sort_by_cached_key(|&child_id| {
                        let child = &self.nodes[child_id];
                        (!child.is_dir(), child.name().to_lowercase())
                    });
                    if let Some(node) = self.nodes.get_mut(node_id) {
                        node.children = children.clone();
                    }
                    for child_id in children {
                        self.sort_nodes_recursively(Some(child_id));
                    }
                }
            }
        }
    }

    /// Helper to get the name of the syntax theme currently in use.
    pub(super) fn get_current_syntax_theme_name(&self) -> String {
        match self.config.theme.as_str() {
            "dark" => preview::DEFAULT_DARK_THEME.to_string(),
            "light" => preview::DEFAULT_LIGHT_THEME.to_string(),
            _ => match dark_light::detect() {
                Ok(dark_light::Mode::Dark) => preview::DEFAULT_DARK_THEME.to_string(),
                _ => preview::DEFAULT_LIGHT_THEME.to_string(),
            },
        }
    }
}
