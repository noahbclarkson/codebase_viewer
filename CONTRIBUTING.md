# Contributing to Codebase Viewer RS

Thank you for your interest in contributing! We welcome bug reports, feature requests, and pull requests.

## Reporting Issues

* **Bug Reports:** If you find a bug, please search the existing [GitHub Issues](https://github.com/noahbclarkson/codebase_viewer/issues) to see if it has already been reported. If not, create a new issue, providing:
  * A clear and descriptive title.
  * Steps to reproduce the bug.
  * What you expected to happen.
  * What actually happened (including error messages or screenshots if applicable).
  * Your operating system and the application version (`cargo --version` and app version from About dialog).
* **Feature Requests:** If you have an idea for a new feature or improvement, please open an issue to discuss it first. Describe the feature, why it's useful, and potential implementation ideas.

## Submitting Pull Requests

1. **Fork the Repository:** Create your own fork of the project on GitHub.
2. **Clone Your Fork:** `git clone https://github.com/YOUR_USERNAME/codebase_viewer.git` (Replace `YOUR_USERNAME` with your GitHub username)
3. **Create a Branch:** `git checkout -b feature/your-feature-name` or `bugfix/issue-123`. Use a descriptive branch name.
4. **Make Changes:** Implement your feature or bugfix.
5. **Code Style:** Ensure your code is formatted according to the standard Rust style using `cargo fmt --all`.
6. **Linting:** Ensure your code passes lint checks using `cargo clippy --all-targets --all-features -- -D warnings`. Address any warnings reported by Clippy.
7. **Testing:** If you add new functionality, please add corresponding tests using `#[test]`. Ensure all tests pass with `cargo test --all-features`.
8. **Security Audit:** Run `cargo audit` to check for known security vulnerabilities in dependencies. Address any critical vulnerabilities found.
9. **Performance:** Consider the performance implications of your changes, especially in areas like directory scanning, UI responsiveness, and report generation.
10. **Commit Changes:** Commit your changes with clear and concise commit messages. Reference the relevant issue number if applicable (e.g., `Fix #123: Prevent crash when opening empty directory`).
9. **Push Branch:** `git push origin feature/your-feature-name`
10. **Open Pull Request:** Go to the original repository on GitHub and open a Pull Request from your branch to the `main` branch. Provide a clear description of your changes in the PR description.

## Development Setup

* Install Rust: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install) (Version 1.77 or later recommended).
* Clone the repository.
* Build: `cargo build`
* Run: `cargo run`
* Test: `cargo test`
* Format: `cargo fmt --all`
* Lint: `cargo clippy --all-targets --all-features -- -D warnings`
* Audit: `cargo audit` (Install via `cargo install cargo-audit` if needed. Run periodically.)

## License

By contributing, you agree that your contributions will be licensed under the same dual MIT OR Apache-2.0 license as the project itself. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
