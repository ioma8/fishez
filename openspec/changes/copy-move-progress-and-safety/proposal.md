## Why

`copy_items`/`move_items` (`src/application/use_cases/file_copy.rs`, `file_move.rs`) run synchronously on the UI thread when the user confirms a copy/move destination in `handle_copy_dest_input`/`handle_move_dest_input` (`src/presentation/input_handler.rs:949-1018`). For any non-trivial file or batch, the TUI freezes with no feedback until the operation completes. Worse, neither `copy_one` nor `move_one` checks whether the destination path already exists — `fs_port.copy_file` and `fs_port.rename` will silently overwrite existing files, and a partially-copied directory tree copies over an existing one silently merging/replacing contents. Total Commander users expect a progress dialog with running totals and an explicit overwrite prompt (skip/overwrite/rename/overwrite-all/skip-all) before any existing file is touched.

## What Changes

- Copy and move operations run on a background thread; the main event loop polls for progress updates (reusing the existing 100ms poll cadence in `event_loop.rs`) and renders a progress overlay (current file, N of M items, bytes copied) instead of blocking.
- Before overwriting any existing destination file, the operation pauses and shows a conflict prompt: **Overwrite** / **Skip** / **Overwrite All** / **Skip All** / **Cancel**. Directories merge (recurse into existing dirs) rather than silently replacing.
- Users can cancel an in-progress copy/move (`Esc`), which stops the background thread after the current file and leaves already-copied files in place (no rollback).
- Failures on individual files (permission denied, disk full, etc.) are reported per-file without aborting the whole batch, then summarized at the end ("Copied 42/45, 3 failed").
- **BREAKING**: `FileSystemPort::copy_file` / `rename` signatures gain no new required params for callers outside file_copy/file_move, but `copy_items`/`move_items` change from synchronous `Result<(), String>` returns to a channel-based/callback-driven API — all call sites in `input_handler.rs` must be updated.

## Capabilities

### New Capabilities
- `file-transfer-progress`: background execution of copy/move with progress reporting and cancellation surfaced in the TUI.
- `file-transfer-conflicts`: overwrite-conflict detection and resolution (overwrite/skip/overwrite-all/skip-all/cancel) for copy and move operations, including directory merge semantics.

### Modified Capabilities
(none — no existing `openspec/specs/` capabilities yet)

## Impact

- `src/application/use_cases/file_copy.rs`, `file_move.rs`: rewritten to emit progress/conflict events instead of running to completion synchronously.
- `src/presentation/input_handler.rs:949-1018`: `handle_copy_dest_input`/`handle_move_dest_input` spawn a worker instead of calling `copy_items`/`move_items` inline.
- `src/presentation/event_loop.rs`: poll loop (`event_loop.rs:54`) gains a channel-receive step per tick to pull progress/conflict events and drive redraws.
- `src/presentation/terminal/overlays.rs`: new `draw_progress_overlay` / `draw_conflict_prompt`, modeled on the existing `draw_delete_prompt` (`overlays.rs:33`).
- `src/application/state.rs`: `CopyMoveState` (or a new state struct) needs fields for in-flight job handle/progress/pending conflict.
- No new external dependencies expected — `std::thread` + `std::sync::mpsc` cover the background job and progress channel.
