# Changelog

All notable changes to this project will be documented in this file.  
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

_Nothing yet._

[Unreleased]: https://github.com/noahbclarkson/codebase_viewer/compare/v0.1.1...HEAD

---

## [0.1.1] - 2025-04-28

### Added
- **SVG preview support** via the `resvg` crate.  
- **Batch scanner messages** (`AddNodes`) to reduce UI overhead on huge codebases.  
- **Path-to-ID hash map** for O(1) parent look-ups during scans.

### Changed
- **Scanner performance** improvements (batched delivery + hash map).  
- **Release workflow**: simplified build matrix, unified archive naming, automatic version detection, LTO flags.  
- **CI workflow**: fails on Clippy warnings, runs `cargo audit`, installs **cross** only when needed.  
- **Logging**: streamlined formatting, richer trace output.  
- **Cargo.toml**: bumped to `0.1.1`; added `resvg = 0.45.1`.

### Fixed
- Orphan-node edge cases that sometimes left items out of the tree view.  
- Windows CI now ensures MinGW is present.

### Removed
- Generated `codebase_viewer_report.md` (added to `.gitignore`).  
- Explicit revision pin for **cross** install.

[0.1.1]: https://github.com/noahbclarkson/codebase_viewer/releases/tag/v0.1.1

---

## [0.1.0] - 2025-04-25

### Added

* Initial public release.
* Core functionality: Directory scanning, tree view, file preview (text, images), report generation (Markdown, HTML, Text).
* Configuration persistence (`config.json`).
* Selection persistence (Save/Load Selection to JSON).
* Basic UI elements: Menu bar, status bar, tree panel, preview panel.
* Dialogs: Preferences, Report Options, About, Keyboard Shortcuts.
* File icons in the tree view based on extension.
* Background processing for scanning and report generation.
* Keyboard shortcuts for common actions.
* Basic CI setup using GitHub Actions (format, lint, test, build).
* Project documentation (`README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`).
* Dual licensing (MIT **or** Apache-2.0).

### Changed

* Refactored `app.rs` for better state management and action handling.
* Improved commenting across the codebase (Rustdoc, inline comments).
* Cleaned up unused code and imports.
* Standardized error handling and logging.
* Refined UI layout and widget usage for consistency.
* Optimized report generation data flow.
* Improved Markdown report formatting to align with common linting rules.
* Updated dependencies to recent compatible versions.
* Made `Cargo.toml` metadata more complete.

### Fixed

* Resolved potential race conditions/borrowing issues with deferred actions.
* Corrected parent state calculation after loading selection.
* Improved handling of orphaned nodes during scanning.
* Ensured UI remains responsive during background tasks.
* Fixed minor layout issues in dialogs and panels.
* Addressed various Clippy warnings.

### Removed

* Removed unused `pulldown-cmark` dependency.
* Removed redundant or unhelpful inline comments.

[0.1.0]: https://github.com/noahbclarkson/codebase_viewer/releases/tag/v0.1.0
