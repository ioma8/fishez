# Viral Release Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make fishez launch-ready by fixing stale docs, adding the smallest table-stakes TUI behaviors, improving preview screenshots, and refreshing the demo.

**Architecture:** Keep changes inside existing layers: adapters in `src/infrastructure`, state/use cases in `src/application`, key handling/rendered help in `src/presentation`, and docs at repo root. No new dependencies; shell out to installed tools where useful.

**Tech Stack:** Rust 2024, crossterm, existing clean architecture modules, optional external `bat`, `vhs` for demo recording.

---

## Files

- Modify: `CHANGELOG.md` for accurate release notes.
- Modify: `README.md` for launch positioning, shortcuts, requirements, and editor wording.
- Modify: `src/infrastructure/open_adapter.rs` for `$EDITOR` support.
- Modify: `src/presentation/shortcuts.rs` and `src/presentation/event_loop.rs` for hidden toggle.
- Modify: `src/application/state.rs`, `src/application/use_cases/navigate.rs`, `src/infrastructure/fs_adapter.rs`, `src/domain/entry.rs` for hidden filtering and stable sorting.
- Modify: `src/application/use_cases/quick_view.rs` for optional `bat` preview.
- Modify: `src/presentation/terminal/renderer.rs` for F1 help text.
- Modify: `demo.tape`; regenerate `demo.gif`.

---

## Task 1: Fix Changelog

**Files:**
- Modify: `CHANGELOG.md`

- [ ] Remove placeholder entries: command palette, batch operations panel, placeholder directory creation/rename, stale known limitations.
- [ ] Fix stale bindings:
  - `Fuzzy search with fd (F6)` -> `Fuzzy search with fd (Ctrl+F)`
  - `Content search with ripgrep (F7)` -> `Content search with ripgrep (Ctrl+R)`
- [ ] Add real current features:
  - `fz` cd-on-exit shell wrapper via `fishez --init`
  - start path support: `fishez ~/projects`
  - safe background copy/move with overwrite prompts
  - shell command overlay with `{1}` and `{@}`
  - OSC52 clipboard path copy
- [ ] Verify:
  ```bash
  rg -n "placeholder|F6\\)|F7\\)|No batch operations|No directory creation" CHANGELOG.md
  ```
  Expected: no stale placeholder or wrong-binding hits.

## Task 2: Add `$EDITOR` Support

**Files:**
- Modify: `src/infrastructure/open_adapter.rs`
- Modify: `src/presentation/shortcuts.rs`
- Modify: `src/presentation/input_handler.rs` if context menu wording still says VS Code
- Modify: `README.md`
- Modify: `src/presentation/terminal/renderer.rs`

- [ ] Add a helper in `open_adapter.rs`:
  ```rust
  fn editor_command_from(value: Option<String>) -> Option<Vec<String>> {
      let value = value?;
      let parts: Vec<String> = value.split_whitespace().map(String::from).collect();
      (!parts.is_empty()).then_some(parts)
  }
  ```
- [ ] In `VsCodeAdapter::open`, try `$EDITOR` first:
  ```rust
  if let Some(parts) = editor_command_from(std::env::var("EDITOR").ok()) {
      let _ = Command::new(&parts[0]).args(&parts[1..]).arg(path).spawn();
      return;
  }
  ```
  Keep the existing VS Code fallback branches unchanged.
- [ ] Rename visible text from “Open in VS Code” to “Open in editor” where user-facing, unless specifically describing fallback behavior.
- [ ] Add unit tests for `editor_command_from`: unset, empty, `vim`, `code -w`.
- [ ] Verify:
  ```bash
  cargo test open_adapter
  cargo check
  ```

## Task 3: Add Hidden-Files Toggle

**Files:**
- Modify: `src/application/state.rs`
- Modify: `src/application/use_cases/navigate.rs`
- Modify: `src/presentation/shortcuts.rs`
- Modify: `src/presentation/event_loop.rs`
- Modify: `src/presentation/terminal/renderer.rs`
- Modify: `README.md`

- [ ] Add `show_hidden: bool` to `AppState`, default `true` to preserve current behavior.
- [ ] Thread `show_hidden` into directory refresh calls, or store the flag on each `PanelState` if that makes the diff smaller.
- [ ] In `refresh_entries`, when hidden files are disabled, filter only real entries:
  ```rust
  entries.into_iter().filter(|entry| !entry.name.starts_with('.'))
  ```
  Keep `..` unaffected.
- [ ] Add keybinding `Ctrl+H` in `shortcuts.rs`.
- [ ] In `event_loop.rs`, flip the flag, refresh both panels, and redraw.
- [ ] Update F1 help and README shortcut table with `Ctrl+H`.
- [ ] Add one unit test in `navigate.rs`: dotfiles are hidden when disabled and visible when enabled.
- [ ] Verify:
  ```bash
  cargo test navigate
  cargo check
  ```

## Task 4: Add Stable Sorting

**Files:**
- Modify: `src/domain/entry.rs`
- Modify: `src/infrastructure/fs_adapter.rs`
- Modify: `src/application/use_cases/navigate.rs`

- [ ] Add `modified: Option<SystemTime>` to `FileEntry`.
- [ ] Populate `modified` in `fs_adapter.rs` from `metadata.modified().ok()`.
- [ ] Sort listed entries inside `refresh_entries` after filtering:
  - directories first
  - then files
  - names case-insensitively
  - keep `..` first
- [ ] Do not add size/date sort modes for launch.
- [ ] Add one unit test in `navigate.rs` covering dirs-first and case-insensitive name order.
- [ ] Verify:
  ```bash
  cargo test navigate
  cargo check
  ```

## Task 5: Improve Text Preview with Optional `bat`

**Files:**
- Modify: `src/application/use_cases/quick_view.rs`
- Modify: `README.md`

- [ ] Before the existing manual read path in `preview_text`, try:
  ```rust
  Command::new("bat")
      .args([
          "--color=always",
          "--plain",
          "--paging=never",
          "--wrap=character",
          "--terminal-width",
          &wrap_width.to_string(),
      ])
      .arg(path)
      .output()
  ```
- [ ] If `bat` succeeds and stdout is non-empty, split stdout into lines, cap to `TEXT_PREVIEW_MAX_LINES`, and return `QuickViewMode::Text { lines, start: 0 }`.
- [ ] On spawn error, non-zero status, or empty stdout, keep the existing highlighter fallback.
- [ ] Mention `bat` as optional in README requirements.
- [ ] Add a unit test for the split/cap helper; do not mock the `bat` process.
- [ ] Manually verify with and without `bat` on PATH:
  ```bash
  cargo run -- .
  ```

## Task 6: Re-Record Demo GIF

**Files:**
- Modify: `demo.tape`
- Modify: `demo.gif`
- Modify: `README.md` if the demo caption changes

- [ ] Update `demo.tape` to show this under-30-second loop:
  - launch `fz ~/projects` or equivalent staged demo directory
  - navigate
  - quick-preview code
  - run `Ctrl+R` content search
  - press F4 to open editor
  - quit back into the directory
- [ ] Use a readable terminal size and font in the tape.
- [ ] Regenerate:
  ```bash
  FISHEZ_BIN=/tmp/fishez_target/release/fishez vhs demo.tape
  ```
- [ ] Verify `demo.gif` is readable in the GitHub README preview.

## Task 7: Clean README

**Files:**
- Modify: `README.md`

- [ ] Lead with:
  ```bash
  cargo install fishez
  eval "$(fishez --init)"
  fz ~/projects
  ```
- [ ] Replace “Open in VS Code” primary wording with “Open in editor”; mention VS Code as fallback.
- [ ] Add a short “Why not yazi/ranger/nnn?” section:
  - zero-config
  - developer defaults
  - `fz` cd-on-exit
  - fd/rg/editor flow
- [ ] Move requirements after the core workflow.
- [ ] Remove or soften claims not shown by the demo.
- [ ] Verify every shortcut row against `src/presentation/shortcuts.rs` and F1 help.

## Final Verification

- [ ] Run:
  ```bash
  cargo fmt
  cargo test
  cargo clippy --all-targets --all-features
  cargo check
  ```
- [ ] Manual smoke test:
  ```bash
  cargo run -- ~/projects
  EDITOR=vim cargo run -- .
  ```
- [ ] Confirm docs:
  ```bash
  rg -n "VS Code|Ctrl\\+F|Ctrl\\+R|Ctrl\\+H|bat|fz|placeholder" README.md CHANGELOG.md
  ```

## Out of Scope

- Plugins.
- Themes.
- Config file.
- Tabs.
- Mouse-first workflows.
- Archive browsing.
- AI features.
- Size/date sorting before launch.
