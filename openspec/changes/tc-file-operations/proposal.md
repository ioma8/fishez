## Why

fishez was built to bring Total Commander muscle memory to the macOS terminal — but the core file operation hotkeys are missing. Without F5/F6/F7/F8, TC refugees hit the first friction point immediately and fall back to their old tools. This change completes the TC-compatible baseline and resolves two key conflicts that arise from adding TC bindings to the existing layout.

## What Changes

### New operations
- **F5** — copy selected file(s) to the opposite pane (dual-pane) or prompt for destination (single-pane)
- **F6** — move selected file(s) to the opposite pane (dual-pane) or prompt for destination (single-pane)
- **Shift+F6** — inline rename of the file/dir under the cursor (macOS TC convention)
- **F7** — create new folder in the current directory
- **F8** — delete selected file(s); safe-trash behaviour identical to existing Ctrl+W; Ctrl+W stays as alias

### Key reassignments (conflict resolution)
- **F6 currently = fd find** → relocated to **Alt+F7** (TC "Find Files" convention)
- **F7 currently = ripgrep** → relocated to **Ctrl+G** (grep mnemonic)

### Discoverability
- Mention `--two-pane` / `-2` in the onboarding banner; single-pane remains the default

## Capabilities

### New Capabilities

- `file-copy`: Copy one or more files/dirs to a destination; in dual-pane the opposite pane's path is the default destination; confirm dialog shows source → dest before executing
- `file-move`: Move one or more files/dirs to a destination; same destination logic as copy; updates the source pane after completion; atomic rename with cross-device fallback
- `inline-rename`: Rename the item under the cursor (Shift+F6) with an editable prompt pre-filled with the current name; Enter commits, Esc cancels
- `new-folder`: Create a new directory in the current pane (F7); prompts for folder name; refreshes pane on success
- `f8-delete`: Trigger the existing delete flow via F8; Ctrl+W remains a working alias

### Modified Capabilities

*(none — no existing spec files exist yet)*

## Impact

- `src/presentation/shortcuts.rs` — add F5, F6, Shift+F6, F7, F8 handlers; update F6/F7 to Alt+F7/Ctrl+G
- `src/presentation/input_handler.rs` — add `handle_rename_input`, `handle_new_folder_input`
- `src/presentation/terminal/overlays.rs` — add prompt overlays for rename, copy/move dest, new folder
- `src/application/use_cases/` — add `file_copy.rs`, `file_move.rs`, `new_folder.rs`
- `src/infrastructure/fs_adapter.rs` — add `copy_file`, `rename`, `create_dir` to `FileSystemPort`
- `src/main.rs` / `src/presentation/event_loop.rs` — thread new modal state through event loop
- `src/presentation/terminal/renderer.rs` — update help entries and footer actions
- `README.md` — update features table and shortcuts section with new and reassigned keys
