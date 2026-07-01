# fishez — Implementation Plan

Six independent items, ordered by ROI. Each is self-contained and can be implemented and committed separately. See `IMPROVEMENTS.md` for rationale. After each item: `cargo test && cargo clippy` must pass; update README shortcut table and F1 help overlay when a keybinding is added.

---

## 1. cd-on-exit (`--cwd-file`)

**Goal:** `fishez --cwd-file /tmp/x` writes the active panel's directory to that file on clean exit, so a shell wrapper can `cd` there.

- `src/main.rs`: parse `--cwd-file <path>` from `env::args()` (value follows the flag). Store as `Option<PathBuf>`.
- After `run(...)` returns in `main()`, if the flag was given: `fs::write(path, state.active_panel().current_path.as_os_str().as_encoded_bytes())`. Ignore write errors. `AppState::active_panel()` exists at `src/application/state.rs:207`; `run` currently takes `&mut state`, so state is still accessible after it returns.
- README: add a "Shell integration" section with:
  ```sh
  fz() {
    local tmp="$(mktemp)"
    fishez --cwd-file "$tmp" "$@"
    local dir="$(cat "$tmp")"; rm -f "$tmp"
    [ -d "$dir" ] && cd "$dir"
  }
  ```
  Note it works for bash and zsh; add a fish variant (`function fz; ...; end`).
- `install.sh`: print the wrapper snippet as a post-install hint (do not auto-edit rc files).
- Test: unit test that the file gets written — construct `AppState`, call the extracted `write_cwd_file(&state, &path)` helper, assert file contents equal the panel path.

## 2. Respect `$EDITOR` on F4

**Goal:** F4 opens the file in `$EDITOR` if set, else VS Code (current behavior).

- `src/infrastructure/open_adapter.rs`, `VsCodeAdapter::open` (rename adapter to `EditorAdapter`; keep the variable name in `main.rs` or rename — mechanical): at the top, if `env::var("EDITOR")` is set and non-empty, run it. Terminal editors (vim, nvim, hx, nano, emacs -nw) must take over the TTY, so this is not `spawn()`:
  - Suspend the TUI: the event loop must handle this — the simplest correct route is a new `Message`/return path that tells the presentation layer to `disable_raw_mode` + leave alternate screen, run `Command::new(editor).arg(path).status()` (blocking), then re-enter alternate screen + `enable_raw_mode` + full redraw. Follow the same pattern the shell-command (`!`) feature already uses for running external commands — reuse its suspend/resume code if it has one; if `!` runs without suspending, add one shared `fn run_in_terminal(cmd: &mut Command, renderer: &mut TerminalRenderer)` used by both.
  - `$EDITOR` may contain arguments (`"code -w"`): split on whitespace, first token is the program.
- If `$EDITOR` unset: keep existing VS Code branches unchanged.
- Test: unit test for the `$EDITOR` parsing (split program/args); manual check with `EDITOR=vim` and unset.

## 3a. Hidden-files toggle (Ctrl+H)

**Goal:** dotfiles hidden by default is NOT current behavior — currently everything shows. Add a toggle; default = show hidden (preserves current behavior).

- `src/application/state.rs`: add `pub show_hidden: bool` to `AppState` (app-wide, not per-panel), default `true`.
- `src/application/use_cases/navigate.rs` `refresh_entries` (already takes `panel`; add a `show_hidden: bool` param or move the flag into `PanelState` — prefer adding the param and threading it from call sites): after `fs.list_dir`, if `!show_hidden`, drop entries whose `name` starts with `'.'` (the `..` parent entry is added separately and unaffected).
- `src/presentation/shortcuts.rs`: add `is_toggle_hidden` for Ctrl+H (pattern-match the existing Ctrl+F/Ctrl+R helpers at lines 169–173).
- `src/presentation/event_loop.rs`: on toggle, flip the flag, `refresh_entries` on both panels, redraw.
- Keep cursor sane: `refresh_entries` already resets cursor to 0 — acceptable.
- Test: unit test that `refresh_entries` with `show_hidden=false` excludes dotfiles and keeps `..`.

## 3b. Sort modes (Ctrl+S cycles name → size → modified)

**Goal:** deterministic sorting, directories always first, three modes cycled by one key. Current listing is raw `read_dir` order.

- `src/domain/entry.rs`: add `pub modified: Option<SystemTime>` to `FileEntry` and to `FileEntry::new` (update all call sites — grep `FileEntry::new`; tests and `navigate.rs` parent entry pass `None`).
- `src/infrastructure/fs_adapter.rs` `list_dir` (line 13): populate `modified` from `entry.metadata().and_then(|m| m.modified()).ok()`. Also populate real `size` (already done) — note dir size stays 0.
- `src/application/state.rs`: add `pub enum SortMode { Name, Size, Modified }` and `pub sort_mode: SortMode` on `AppState`, default `Name`, with a `cycle()` method.
- `navigate.rs` `refresh_entries`: after filtering, sort entries: dirs before files always; then by mode — Name: case-insensitive `name`; Size: descending `size`; Modified: newest first. `..` entry stays at index 0 (it's pushed before the listing; sort only the listed slice).
- Keybinding: Ctrl+S in `shortcuts.rs` + handler in `event_loop.rs` (cycle, refresh both panels, show a notification like `Sort: size` via the existing `panel.notification`).
- Footer/header: optional — skip UI indicator beyond the notification.
- Test: unit test sorting with a fake `FileSystemPort` (see `src/test_support.rs`): dirs first, each mode's order correct, `..` first.

## 4. Syntax highlighting via `bat` shell-out

**Goal:** if `bat` is on PATH, use it for text previews; otherwise keep the existing naive highlighter. Mirrors the existing fd/rg shell-out philosophy.

- `src/application/use_cases/quick_view.rs` `preview_text`: before reading the file manually, try:
  `Command::new("bat").args(["--color=always", "--plain", "--paging=never", "--wrap=character", "--terminal-width", &wrap_width.to_string()]).arg(path).output()`
  - On success (status ok, non-empty stdout): split stdout into lines (`String::from_utf8_lossy`), cap at the same max line count the current reader uses, return `QuickViewMode::Text`.
  - On failure or `bat` missing (spawn error): fall back to the current path (read + `highlight()`).
- Cache the "is bat available" check in a `std::sync::OnceLock<bool>` so it's one `which`-style probe per session.
- Renderer already prints ANSI-styled lines from `highlight()` (it uses crossterm `Stylize`), so ANSI passthrough should work; verify one render manually — if the renderer escapes ANSI, strip this item down to only the fallback and flag it.
- README: mention `bat` as an optional dependency alongside fd/rg.
- Test: manual (`cargo run`, F3 on a .rs file with and without bat on PATH). Unit test only the line-splitting/cap helper.

## 5. Demo GIF + distribution

**Goal:** a reproducible demo GIF at the top of the README and one-command installs.

- Add `demo.tape` (vhs script, https://github.com/charmbracelet/vhs) at repo root: ~25s scenario — launch `fishez -2`, navigate, type-to-filter, F3 preview a code file, Space multi-select, F5 copy with progress, Ctrl+F fuzzy search, quit. Output `demo.gif`.
- Generate: `vhs demo.tape` (requires vhs + ttyd installed; document in `RELEASE_GUIDE.md`). Commit the GIF (or upload to a GitHub release and hot-link to keep repo small — prefer the release-asset link).
- README: embed the GIF directly under the logo, before any text.
- crates.io: verify `cargo publish --dry-run` passes. Blocker: the git dependency `jpgfromrawlib` — crates.io forbids git deps. Either publish `jpgfromraw` to crates.io first, or make it optional behind a `raw` feature excluded from the published build. Add missing manifest fields (`description`, `license`, `repository`, `keywords = ["tui","file-manager","terminal"]`, `categories = ["command-line-utilities","filesystem"]`).
- Homebrew: create `ioma8/homebrew-tap` repo with a formula pulling the GitHub release binaries (CI already builds multiplatform per `MULTIPLATFORM_BUILD.md` — reuse those artifacts). README: `brew install ioma8/tap/fishez`.
- Submissions (manual, post-release): awesome-tuis PR, r/rust, r/commandline, HN Show HN. Not automatable — list them in `RELEASE_GUIDE.md` as a launch checklist.

## 6. Doc hygiene

- README shortcut table: fix fuzzy search `Alt+F7` → `Ctrl+F`, content search `Ctrl+G` → `Ctrl+R` (bindings live at `src/presentation/shortcuts.rs:169-173` — verify every table row against `shortcuts.rs` while there). Add new bindings from items 1–3 (`fz` wrapper, Ctrl+H, Ctrl+S).
- CHANGELOG.md: remove "(placeholder)" entries (command palette, batch operations panel, directory rename placeholder); fix stale "Fuzzy search with fd (F6)" / "Content search with ripgrep (F7)" lines; add an Unreleased section entry per item shipped from this plan.
- F1 help overlay (`src/presentation/terminal/overlays.rs`): verify it matches `shortcuts.rs`; add new bindings.
- If README claims "syntax highlighting" ships before item 4 lands, soften to "code-aware preview" — or land item 4 first.
