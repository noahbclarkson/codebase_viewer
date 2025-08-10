# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

```bash
# Build the project in release mode (recommended for performance)
cargo run --release

# Build in debug mode with faster compilation
cargo run

# Format code (always run before committing)
cargo fmt --all

# Run strict linting (must pass with no warnings)
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test --all-features

# Security audit (run periodically)
cargo audit
```

## Code Architecture

This is a cross-platform desktop GUI application built with Rust and egui. The app is structured as follows:

### Core Application Structure
- **src/main.rs**: Entry point that initializes logging, eframe, and syntax highlighting assets
- **src/app/**: Main application logic implementing eframe::App trait
  - `state.rs`: Core `CodebaseApp` struct and state management  
  - `actions.rs`: Implements all `AppAction` variants (file operations, UI state changes)
  - `message_handling.rs`: Processes background task messages
  - `helpers.rs`: Miscellaneous app helper functions
- **src/model.rs**: Core data structures (`FileNode`, `Check` tri-state, `FileId`)

### Key Modules
- **src/fs/**: File system operations
  - `scanner.rs`: Background directory scanner using `ignore` crate for .gitignore support
  - `file_info.rs`: File metadata structures
  - `stats.rs`: Statistics collection during scans
- **src/ui/**: egui UI components
  - `tree_panel.rs`: Left panel with file tree and search
  - `preview_panel.rs`: Right panel for file content preview  
  - `dialogs.rs`: Preferences, Report Options, About windows
  - `menu_bar.rs` & `status_bar.rs`: Top menu and bottom status
- **src/report/**: Report generation in multiple formats
  - `generator.rs`: Core report data collection
  - `html.rs`, `markdown.rs`, `text.rs`: Format-specific exporters
- **src/preview.rs**: File content preview with syntax highlighting (syntect)
- **src/config.rs**: Application configuration management
- **src/selection.rs**: Save/load tree selection state to JSON

### Threading Architecture
- Main thread runs the egui UI loop at ~60fps
- Background threads handle:
  - Directory scanning (parallel via `ignore` crate + rayon)
  - Report generation 
  - File content loading for preview
- Communication via `crossbeam-channel` for MPSC messaging
- Actions are queued in `deferred_actions` and processed after UI drawing

### Key Dependencies
- **egui/eframe**: Immediate mode GUI framework
- **syntect**: Syntax highlighting using Sublime Text grammars
- **ignore**: Fast directory traversal respecting .gitignore
- **rayon**: Data parallelism for scanning
- **serde**: Configuration and selection serialization
- **rfd**: Native file dialogs
- **image + resvg**: Image and SVG preview support

## Development Guidelines

### Code Quality
- Strict clippy lints enforced via `clippy.toml` configuration
- Uses workspace lints in Cargo.toml with pedantic + nursery + cargo groups
- Disallows unsafe code, print macros, std::sync primitives (use parking_lot instead)
- Must pass `cargo clippy --all-targets --all-features -- -D warnings`

### Performance Considerations
- Always test with `--release` for realistic performance on large codebases
- Directory scanning is multi-threaded and respects .gitignore files
- UI remains responsive during long operations via background threading
- Preview caching prevents repeated file reads

### File Structure Conventions
- UI components are organized by panel/window in `src/ui/`
- Background tasks communicate via typed message enums in `src/task.rs`
- App state is centralized in `CodebaseApp` with action-based mutations
- File system operations are abstracted in `src/fs/` module

### Key Configuration
- User config stored in OS-native directories via `dirs-next`
- Supports themes (light/dark/system), file size limits, export preferences
- Recent projects list maintained automatically
- Selection state can be saved/loaded as JSON files