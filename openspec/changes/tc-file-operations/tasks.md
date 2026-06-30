## 1. Relocate conflicting keys (do this first)

- [x] 1.1 In `src/presentation/shortcuts.rs` `handle_search`: change `F(6)` → `Alt+F7` (`KeyCode::F(7)` + `KeyModifiers::ALT`) for fd find
- [x] 1.2 Change `F(7)` → `Ctrl+G` (`KeyCode::Char('g')` + `KeyModifiers::CONTROL`) for ripgrep
- [x] 1.3 Update `default_help_entries()` in `renderer.rs`: `("F6", "Find (fd)")` → `("Alt+F7", "Find (fd)")`, `("F7", "RipGrep")` → `("Ctrl+G", "RipGrep")`
- [x] 1.4 Update `footer_actions()` for `PanelMode::Normal` to reflect new keys
- [x] 1.5 Update README features table and All Shortcuts section for the reassigned keys
- [x] 1.6 `cargo check` — confirm no regressions before adding new features

## 2. Infrastructure — FileSystemPort

- [x] 2.1 Add `fn rename(from: &Path, to: &Path) -> io::Result<()>` to `FileSystemPort` trait
- [x] 2.2 Implement on `StdFileSystem` using `std::fs::rename`
- [x] 2.3 Add `fn copy_file(from: &Path, to: &Path) -> io::Result<()>` to `FileSystemPort`
- [x] 2.4 Implement on `StdFileSystem` using `std::fs::copy`
- [x] 2.5 Add `fn create_dir(path: &Path) -> io::Result<()>` to `FileSystemPort`
- [x] 2.6 Implement on `StdFileSystem` using `std::fs::create_dir`

## 3. Use Cases

- [x] 3.1 Create `src/application/use_cases/file_copy.rs` — `copy_items(sources: &[PathBuf], dest_dir: &Path, fs: &impl FileSystemPort) -> Result<(), String>` with recursive directory support
- [x] 3.2 Create `src/application/use_cases/file_move.rs` — `move_items(sources: &[PathBuf], dest_dir: &Path, fs: &impl FileSystemPort) -> Result<(), String>`; try `rename` first, fall back to copy+delete on cross-device error (EXDEV); do not delete source until copy is complete
- [x] 3.3 Create `src/application/use_cases/new_folder.rs` — `create_folder(name: &str, parent: &Path, fs: &impl FileSystemPort) -> Result<(), String>`
- [x] 3.4 Register all three in `src/application/use_cases/mod.rs`

## 4. Modal State — event loop threading

- [x] 4.1 Add `CopyMoveState { sources: Vec<PathBuf>, dest: String }` to `src/presentation/input_handler.rs` (moved here to avoid circular deps)
- [x] 4.2 Add locals to `main()`: `rename_input: Option<String>`, `new_folder_input: Option<String>`, `copy_dest: Option<CopyMoveState>`, `move_dest: Option<CopyMoveState>`
- [x] 4.3 Thread all four through `run()`, `route_input()`, `handle_modal_overlays()` (follow `shell_command`/`shell_history` pattern)

## 5. Input Handlers

- [x] 5.1 Add `pub fn handle_rename_input(event, rename_input: &mut Option<String>, fs: &StdFileSystem, state: &mut AppState) -> bool` — Char/Backspace edits; Enter commits via `fs.rename`; Esc cancels; refresh panel + move cursor to renamed item on success; notification on error
- [x] 5.2 Add `pub fn handle_new_folder_input(event, new_folder_input: &mut Option<String>, fs: &StdFileSystem, state: &mut AppState) -> bool` — same editing keys; Enter calls `new_folder::create_folder`; pane refreshes with cursor on new dir
- [x] 5.3 Add `pub fn handle_copy_dest_input(event, copy_dest: &mut Option<CopyMoveState>, fs: &StdFileSystem, state: &mut AppState) -> bool` — Enter runs `file_copy::copy_items`; refresh both panes
- [x] 5.4 Add `pub fn handle_move_dest_input(event, move_dest: &mut Option<CopyMoveState>, fs: &StdFileSystem, state: &mut AppState) -> bool` — Enter runs `file_move::move_items`; refresh both panes

## 6. Shortcuts — keybinding entry points

- [x] 6.1 Add **F8** handler in `shortcuts.rs` — same logic as Ctrl+W (set `delete_paths` from selection)
- [x] 6.2 Add **Shift+F6** handler — set `rename_input = Some(current_item_name)`; return `Some(true)` (check `KeyModifiers::SHIFT` + `KeyCode::F(6)`)
- [x] 6.3 Add **F5** handler — build `CopyMoveState { sources, dest: opposite_pane_or_empty }`; set `copy_dest`; return `Some(true)`
- [x] 6.4 Add **F6** handler (plain, no modifier) — same as F5 but set `move_dest`
- [x] 6.5 Add **F7** handler — set `new_folder_input = Some(String::new())`; return `Some(true)`
- [x] 6.6 Verify Shift+F6 check comes BEFORE plain F6 check in the dispatch chain to avoid shadowing

## 7. Overlays — prompt rendering

- [x] 7.1 Add `draw_with_rename` / `draw_rename_prompt` in `overlays.rs` — `"Rename: <name>"` in prompt bar
- [x] 7.2 Add `draw_with_new_folder` / `draw_new_folder_prompt` — `"New folder: <name>"`
- [x] 7.3 Add `draw_with_copy_dest` / `draw_copy_dest_prompt` — `"Copy to: <dest>"`
- [x] 7.4 Add `draw_with_move_dest` / `draw_move_dest_prompt` — `"Move to: <dest>"`
- [x] 7.5 Wire all four into `redraw_current_view` in `event_loop.rs`
- [x] 7.6 Wire all four into `handle_modal_overlays`

## 8. Help & Documentation

- [x] 8.1 Add `("F5", "Copy to")`, `("F6", "Move to")`, `("Shift+F6", "Rename")`, `("F7", "New folder")`, `("F8", "Delete")` to `default_help_entries()` in `renderer.rs`
- [x] 8.2 Add F5/F6/Shift+F6/F7/F8 to `footer_actions()` for `PanelMode::Normal`
- [x] 8.3 Add all new operations to the README features table (done in group 1)
- [x] 8.4 Add all new operations and reassigned keys to README All Shortcuts section (done in group 1)

## 9. Verification

- [ ] 9.1 `cargo check` passes with no new warnings
- [ ] 9.2 `cargo test` passes
- [ ] 9.3 Verify Alt+F7 triggers fd find (not blocked by new F7 handler)
- [ ] 9.4 Verify Ctrl+G triggers ripgrep
- [ ] 9.5 Verify Shift+F6 opens rename prompt (not move prompt)
- [ ] 9.6 Verify plain F6 opens move prompt
- [ ] 9.7 Smoke test: rename (Shift+F6), copy (F5), move (F6), new folder (F7), delete (F8 and Ctrl+W)
- [ ] 9.8 Dual-pane: F5/F6 pre-fills opposite pane path; single-pane: empty prompt
- [ ] 9.9 Cross-device move fallback: test moving between two different volumes if available
