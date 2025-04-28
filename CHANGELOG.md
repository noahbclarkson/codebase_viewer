# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

* SVG file preview support using the `resvg` crate.

### Changed

* Optimized scanner message handling by replacing O(N) path lookup with O(1) HashMap lookup, significantly improving performance for large directories.
[Unreleased]: https://github.com/noahbclarkson/codebase_viewer/compare/v0.1.0...HEAD
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
* Dual licensing (MIT OR Apache-2.0).

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
