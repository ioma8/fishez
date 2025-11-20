# Codebase Improvements & Feature Ideas

## Technical Enhancements
- Resolve the unused image quick-view helpers in `src/terminal_ui.rs` by either wiring them into the render loop or trimming them, keeping the module warning-free and easier to navigate.
- Extract a shared shortcut binding registry so feature modules (e.g., `src/features/find_feature.rs`) can declare their keys declaratively, reducing duplication in `main.rs` and easing future keymap changes.
- Introduce structured logging via `tracing` + `tracing_subscriber` to replace the ad-hoc logger, enabling leveled logs and optional JSON output for debugging.
- Add integration tests that spin up a temporary directory and validate navigation/filter logic through the `FilesView` API to guard against regressions in file ordering and filtering.
- Cleanly separate UI drawing from state mutation in `terminal_ui.rs` by moving render-only helpers into a lightweight view layer, simplifying future theming work.

## New Feature Opportunities
- Implement directory creation/rename shortcuts (completing the TODOs in `main.rs`) using a contextual modal so users can manage files without leaving the TUI.
- Add a pluggable preview pipeline that detects common file types (Markdown, JSON, images) and renders tailored previews, leveraging existing quick view infrastructure.
- Offer a fuzzy-jump palette that indexes recent paths using an optional `zoxide` integration (already hinted in code), speeding navigation to frequently used directories.
- Provide a batch actions panel for the multiselect feature, covering copy/move/delete in one flow and showing progress with cancellable operations.
- Ship a command palette with searchable actions ("Open in VS Code", "Toggle favorites") exposed through a single shortcut, improving discoverability of feature modules.

## DX & Release Polish
- Automate smoke tests in CI with `cargo check`, `cargo clippy`, and `cargo fmt -- --check`, and publish coverage snapshots from `cargo tarpaulin` or similar.
- Document manual QA scripts for image previews and external binary fallbacks in `FLOW.md`, keeping contributors aligned on cross-platform expectations.
- Package prebuilt binaries via GitHub Releases and provide Homebrew/tap instructions for easier onboarding.
