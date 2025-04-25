# Codebase Viewer

Codebase Viewer is a cross-platform desktop application—written entirely in Rust—that lets you **scan, explore, and document large codebases** with millisecond-level responsiveness.  
The UI is built with [egui](https://github.com/emilk/egui) via *eframe*, giving you a native-feeling window on Windows, macOS, Linux, and the web.

## ✨ Key Features

| Capability                           | Details                                                                                                                                                                 |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Blazing-fast scans**               | Parallel directory walking powered by the **ignore** crate’s `WalkBuilder`, which respects `.gitignore`, global Git excludes, and hidden-file masks. |
| **Live tree UI**                     | Immediate-mode GUI rendered by egui/eframe; every file appears as soon as it’s discovered, even while the scan is still running.                     |
| **Selective exports**                | Keep the *full* directory context but choose exactly which files’ contents go into HTML, Markdown, or plain-text reports—ideal for LLM ingestion.                       |
| **Syntax-highlighted preview**       | On-the-fly colouring courtesy of **syntect**, using Sublime-Text grammars.                                                                          |
| **Native dialogs & theme awareness** | File/dir pickers via **rfd** and automatic light/dark detection via **dark-light**.                                              |
| **Cross-thread messaging**           | Non-blocking updates sent through **crossbeam-channel** for MPMC performance.                                                                                     |
| **Human-readable sizes**             | Byte counts formatted with **humansize**.                                                                                                           |
| **Config persistence**               | Settings stored in the OS-native config directory obtained with **dirs-next**.                                                                     |

## 🚀 Quick Start

```bash
# 1. Clone and build in release mode
git clone https://github.com/yourusername/codebase-viewer-rs.git
cd codebase-viewer-rs
cargo run --release

# 2. Open a project
File ▸ Open Directory …   # or use the recent-projects list

# 3. Explore & select
– Navigate the tree  
– Tick files/dirs you want included  

# 4. Generate documentation
File ▸ Generate Report …  
Choose Markdown / HTML / Text and hit **Generate**
```

> **System requirements**: Any modern OS with Rust 1.77+; the app spawns background threads using **Rayon’s** `spawn_fifo` for breadth-first task ordering where available.

## 🔧 Configuration

A JSON config is auto-saved to:

```text
$HOME/.config/codebase_viewer_rs/config.json   # Linux
%APPDATA%\\codebase_viewer_rs\\config.json     # Windows
~/Library/Application Support/codebase_viewer_rs/config.json  # macOS
```

Key fields:

| Key                     | Purpose                                                             |
| ----------------------- | ------------------------------------------------------------------- |
| `theme`                 | `"light"`, `"dark"`, or `"system"`                                  |
| `auto_expand_limit`     | Auto-expand dirs whose total file count ≤ this value                |
| `max_file_size_preview` | Size threshold (bytes) before preview/export refuses to read a file |
| `export_*`              | Toggle stats or file-contents in generated reports                  |

## 🏗️ Architecture Overview

```text
src/
├── app.rs        # Top-level eframe::App; orchestrates everything
├── fs/           # Fast scanner & FileInfo metadata (ignore + walkdir)
├── ui/           # egui panels (tree, preview, dialogs, status bar)
├── preview.rs    # Syntax highlighting (syntect) ➜ egui::LayoutJob
├── report/       # Markdown / HTML / Text generators
├── selection.rs  # Save / load checked-state snapshots (.json)
└── config.rs     # Serde serialisable settings (dirs-next location)
```

* All long-running work (directory walking, preview highlighting) happens in background threads; the GUI remains 60 fps responsive thanks to non-blocking `crossbeam-channel` pushes.
* Walks respect `.gitignore` and other VCS filters via `WalkBuilder`.
* Reports embed the *full* tree followed by the **selected** subtree and optionally syntax-highlighted file bodies, giving LLMs the requisite context without quota blow-outs.

## 🖼️ Screenshots

*(coming soon – run the app and press `Ctrl+G` to export an HTML report you can screen-grab!)*

## 🛠️ Development

```bash
# Lint, test, and run with hot-reload
cargo clippy --all-targets --all-features
cargo test
cargo run
```

### Feature flags

| Flag                                 | Effect                                           |
| ------------------------------------ | ------------------------------------------------ |
| `--features "persistence"` (default) | Enables automatic window-state saving via eframe |
| *(none yet)*                         | Add your own!                                    |

### Performance tips

* Build with `--release`; the scanner uses SIMD-aware pattern-matching inside **ignore** for peak throughput.  
* On large monorepos you can uncheck **Include file contents** in the report dialog to avoid reading GiB-sized binaries.

## 🤝 Contributing

1. Fork & clone  
2. `git checkout -b feature/my-feature`  
3. Make changes + run `cargo fmt`  
4. Submit a PR—remember to explain **why** your change matters.

See [CONTRIBUTING.md](CONTRIBUTING.md) for coding-style & commit-message guidelines (TBD).

## 📜 License

Distributed under the MIT or Apache-2.0 dual license—pick whichever suits your needs.

---

*This project proudly demonstrates how ergonomic a fully native Rust GUI stack has become with egui/eframe.*
