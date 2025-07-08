//! Handles saving and loading the selection state of the file tree.

use crate::model::{Check, FileId, FileNode};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, BufWriter}, // Removed unused ErrorKind
    path::{Path, PathBuf},
};

/// Structure for serializing selection data to JSON.
#[derive(Serialize, Deserialize, Debug)]
struct SelectionData {
    /// Version of the application that saved the file.
    app_version: String,
    /// Timestamp when the selection was saved (RFC3339 format).
    timestamp: String,
    /// The absolute root path of the directory when the selection was saved.
    root_path: String,
    /// Map where keys are relative paths from the root, and values are the `Check` state.
    selection: HashMap<String, Check>,
}

/// Saves the current selection state of the tree nodes to a JSON file.
///
/// Only saves the state of nodes relative to the `root_id`.
///
/// # Arguments
/// * `nodes` - Slice containing all `FileNode`s in the application's arena.
/// * `root_id` - The `FileId` of the root node of the current tree.
/// * `root_path` - The absolute path of the root directory.
/// * `file_path` - The path where the JSON selection file should be saved.
///
/// # Returns
/// * `Ok(())` on successful save.
/// * `Err(anyhow::Error)` on failure (e.g., I/O error, serialization error).
pub fn save_selection_to_file(
    nodes: &[FileNode],
    root_id: Option<FileId>,
    root_path: &Path,
    file_path: &Path,
) -> anyhow::Result<()> {
    let root_id = match root_id {
        Some(id) => id,
        None => {
            log::warn!("Attempted to save selection, but no root node ID is set.");
            return Ok(()); // Nothing to save if no root
        }
    };

    log::info!("Collecting selection state for saving...");
    let mut selection_map = HashMap::new();

    // Start recursion from the children of the root node.
    // The root node itself isn't typically saved by relative path,
    // unless a special key like "." is used (currently not implemented).
    if let Some(root_node) = nodes.get(root_id) {
        let root_relative_path = PathBuf::new(); // Start with an empty relative path
        for &child_id in &root_node.children {
            collect_selection_recursive(nodes, child_id, &root_relative_path, &mut selection_map);
        }
        // Optionally save root state itself if needed:
        // selection_map.insert(".".to_string(), root_node.state);
    } else {
        log::error!("Root node ID {root_id} is invalid during save selection.");
        return Err(anyhow::anyhow!("Invalid root node ID during save."));
    }

    log::info!("Collected state for {} nodes.", selection_map.len());

    // Create the data structure to serialize
    let data = SelectionData {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Local::now().to_rfc3339(), // Use standard RFC3339 timestamp
        root_path: root_path.display().to_string(),
        selection: selection_map,
    };

    // Serialize and write to file
    log::info!("Saving selection state to {}", file_path.display());
    let file = BufWriter::new(File::create(file_path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to create selection file '{}': {}",
            file_path.display(),
            e
        )
    })?);
    serde_json::to_writer_pretty(file, &data)
        .map_err(|e| anyhow::anyhow!("Failed to serialize or write selection data: {}", e))?;

    log::info!("Selection state saved successfully.");
    Ok(())
}

/// Recursive helper to collect the `Check` state of nodes relative to the root.
fn collect_selection_recursive(
    nodes: &[FileNode],
    node_id: FileId,
    current_relative_path: &Path, // Path relative to the *parent*
    selection_map: &mut HashMap<String, Check>,
) {
    let node = match nodes.get(node_id) {
        Some(n) => n,
        None => return, // Should not happen in a valid tree
    };

    // Calculate the relative path for *this* node
    let node_name = node.name();
    let relative_path = current_relative_path.join(node_name);
    // Convert path to string, handling potential non-UTF8 names lossily
    let relative_path_str = relative_path.to_string_lossy().to_string();

    // Store the node's state using its relative path as the key
    selection_map.insert(relative_path_str, node.state);

    // Recurse into children if it's a directory
    if node.is_dir() {
        for &child_id in &node.children {
            // Pass the *current* node's relative path to the children
            collect_selection_recursive(nodes, child_id, &relative_path, selection_map);
        }
    }
}

/// Loads selection state from a JSON file and applies it to the current tree nodes.
///
/// Matches nodes based on their relative paths from the root.
///
/// # Arguments
/// * `nodes` - Mutable slice containing all `FileNode`s in the application's arena.
/// * `root_id` - The `FileId` of the root node of the current tree.
/// * `file_path` - The path to the JSON selection file to load.
///
/// # Returns
/// * `Ok(String)` containing the root path stored in the loaded selection file.
/// * `Err(anyhow::Error)` on failure (e.g., I/O error, deserialization error, missing root).
pub fn load_selection_from_file(
    nodes: &mut [FileNode],
    root_id: Option<FileId>,
    file_path: &Path,
) -> anyhow::Result<String> {
    log::info!("Loading selection state from {}", file_path.display());

    // Read and deserialize the file
    let file = BufReader::new(File::open(file_path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to open selection file '{}': {}",
            file_path.display(),
            e
        )
    })?);
    let data: SelectionData = serde_json::from_reader(file).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse selection file '{}': {}",
            file_path.display(),
            e
        )
    })?;
    let saved_root_path = data.root_path.clone();
    log::info!(
        "Selection file (v{}) loaded successfully. Saved for root: {}",
        data.app_version,
        saved_root_path
    );

    let root_id = match root_id {
        Some(id) => id,
        None => {
            log::warn!(
                "Attempted to load selection, but no root node ID is set in the current tree."
            );
            // Return the saved path, but indicate that nothing was applied
            return Ok(saved_root_path);
        }
    };

    // --- Apply Selection ---
    // 1. Build a map from relative path string to FileId for the *current* tree
    log::debug!("Building path map of current tree for applying selection...");
    let mut path_to_id_map: HashMap<String, FileId> = HashMap::new();
    if let Some(root_node) = nodes.get(root_id) {
        let root_relative_path = PathBuf::new();
        for &child_id in &root_node.children {
            build_path_map_recursive(nodes, child_id, &root_relative_path, &mut path_to_id_map);
        }
        // If root state was saved with ".", map it:
        // path_to_id_map.insert(".".to_string(), root_id);
    } else {
        log::error!("Root node ID {root_id} is invalid during load selection.");
        return Err(anyhow::anyhow!("Invalid root node ID during load."));
    }
    log::debug!("Path map built with {} entries.", path_to_id_map.len());

    // 2. Iterate through the loaded selection data and apply states
    log::debug!("Applying loaded selection states...");
    let mut applied_count = 0;
    let mut not_found_count = 0;
    for (relative_path_str, saved_state) in data.selection {
        // Find the corresponding node ID in the current tree using the relative path
        if let Some(&node_id) = path_to_id_map.get(&relative_path_str) {
            // Get mutable access to the node and update its state
            if let Some(node) = nodes.get_mut(node_id) {
                node.state = saved_state;
                applied_count += 1;
            } else {
                // This should not happen if path_to_id_map is correct
                log::error!(
                    "Mapped node ID {node_id} not found in nodes vector for path '{relative_path_str}'."
                );
            }
        } else {
            // Path from the save file doesn't exist in the current tree
            log::trace!(
                "Path '{relative_path_str}' from selection file not found in current tree."
            );
            not_found_count += 1;
        }
    }

    if not_found_count > 0 {
        log::warn!(
            "{not_found_count} paths from the selection file were not found in the current directory structure."
        );
    }
    log::info!("Applied selection state to {applied_count} nodes.");

    // IMPORTANT: After loading, the parent states (Partial/Checked/Unchecked) might be inconsistent.
    // The caller (`CodebaseApp::perform_load_selection`) is responsible for calling
    // `recalculate_all_parent_states` on the root node to fix this.

    Ok(saved_root_path) // Return the root path stored in the file for comparison/warning
}

/// Recursive helper to build a map from relative path string to `FileId`.
fn build_path_map_recursive(
    nodes: &[FileNode], // Use immutable slice here
    node_id: FileId,
    current_relative_path: &Path,
    path_map: &mut HashMap<String, FileId>,
) {
    let node = match nodes.get(node_id) {
        Some(n) => n,
        None => return,
    };

    let node_name = node.name();
    let relative_path = current_relative_path.join(node_name);
    let relative_path_str = relative_path.to_string_lossy().to_string();

    // Insert the mapping for the current node
    path_map.insert(relative_path_str, node_id);

    // Recurse into children if it's a directory
    if node.is_dir() {
        for &child_id in &node.children {
            build_path_map_recursive(nodes, child_id, &relative_path, path_map);
        }
    }
}
