# Repository Guidelines

## Project Structure & Module Organization
- Application entrypoint lives in `src/main.rs`; shared types sit in `src/lib.rs` and helpers such as `logger.rs` and `terminal_ui.rs`.
- Feature logic is isolated in `src/features/`, with one module per capability (e.g., `find_feature.rs`, `open_feature.rs`). Keep new features under this directory and register them via `src/features/mod.rs`.
- Runtime resources are tracked at the repository root (`favorites.txt`, `err.txt`, `log.txt`). Avoid committing local cache files under `target/`.

## Build, Test, and Development Commands
- `cargo run` launches the TUI file manager from the repository root.
- `cargo check` performs a fast type and borrow check; run it after every change to catch regressions early.
- `cargo fmt` applies Rustfmt conventions before submitting patches; pair with `cargo fmt -- --check` in CI or pre-commit hooks.
- `cargo clippy --all-targets --all-features` surfaces lints that complement `cargo check`.

## Coding Style & Naming Conventions
- Follow the default Rust 2024 edition style: 4-space indentation, snake_case for modules/files, PascalCase for types, and camelCase for local bindings.
- Keep feature modules cohesive: expose a single public `run()`-style entry point and prefer small helper functions within the same file.
- Use the `tracing`-style logging helpers in `logger.rs` consistently when adding diagnostics.

## Testing Guidelines
- Add unit tests within the relevant module using `#[cfg(test)]` submodules; name test functions `test_<behavior>()`.
- For broader workflows, create integration tests under `tests/` and run them with `cargo test`.
- Document manual verification steps for TUI interactions in `FLOW.md` updates when automated coverage is not feasible.

## Commit & Pull Request Guidelines
- Write commit messages with an imperative, English summary in ~50 characters (e.g., `Add ripgrep preview pane`).
- Reference related issues or RFCs in the body when context is needed; include short bullet points for notable side effects.
- PRs should describe the UI impact (screenshots or GIFs if visuals change), list manual test steps, and mention external tool requirements (`fd`, `ripgrep`, `code`).

## Environment Notes
- Ensure `fd`, `ripgrep`, and the VS Code CLI (`code`) are available in `$PATH`; they enable bundled features.
- Favor cross-platform paths (`PathBuf`) and guard OS-specific logic with `cfg` attributes to keep macOS/Linux behavior aligned.
