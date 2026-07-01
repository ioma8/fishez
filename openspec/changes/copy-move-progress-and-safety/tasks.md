## 1. Transfer engine core

- [x] 1.1 Define `TransferEvent` enum (`Progress`, `Conflict`, `Done`) and `ConflictResolution`/`ConflictPolicy` types in `file_copy.rs` (or a new shared `transfer.rs`)
- [x] 1.2 Add a metadata-only pre-count walk that computes total item count for a source list before starting the transfer
- [x] 1.3 Rewrite `copy_one`/`copy_items` to run on a background thread, send `Progress` events per item, and send `Conflict` + block on the reply when the destination already exists
- [x] 1.4 Update directory copy logic: if `dest` exists as a directory, recurse and merge instead of unconditional `create_dir_all`; if `dest` exists as a file, treat as a conflict
- [x] 1.5 Apply `ConflictPolicy` (`AlwaysOverwrite`/`AlwaysSkip`) to skip prompting after the user picks "All"
- [x] 1.6 Collect per-file failures without aborting the batch; include them in the final `Done` event
- [x] 1.7 Add cooperative cancellation (checked between items/recursion steps), wired to a shared flag or control channel

## 2. Move engine on top of transfer core

- [x] 2.1 Rewrite `move_one`/`move_items` to use the same `TransferEvent`/conflict flow as copy for both the rename-fast-path and the cross-device copy+delete fallback
- [x] 2.2 Ensure only successfully-resolved (non-skipped) source entries are removed after a directory move; skipped entries' sources are left in place
- [x] 2.3 Update `is_cross_device` fallback path to route through the shared conflict/progress machinery instead of calling the old synchronous `copy_items`

## 3. UI wiring

- [x] 3.1 Extend `AppState`/`CopyMoveState` (or add a new `TransferState`) to hold the active job's receiver, current progress, and pending conflict
- [x] 3.2 Update `handle_copy_dest_input`/`handle_move_dest_input` (`input_handler.rs:949-1018`) to spawn the worker and store the job handle instead of calling `copy_items`/`move_items` synchronously
- [x] 3.3 In the event loop's poll tick (`event_loop.rs:54`), drain the transfer channel (non-blocking) after the terminal-event poll, updating state and triggering redraws
- [x] 3.4 Add `draw_progress_overlay` in `overlays.rs` (current file name, "N of M") modeled on `draw_delete_prompt`
- [x] 3.5 Add `draw_conflict_prompt` in `overlays.rs` with Overwrite/Skip/Overwrite All/Skip All/Cancel options and key handling
- [x] 3.6 Wire `Esc` during an active transfer to cancellation, and show a "cancelled after N items" notification
- [x] 3.7 Show a completion notification summarizing succeeded/failed counts when `Done` is received

## 4. Tests

- [x] 4.1 Unit tests for conflict detection (file-over-file, dir-over-dir merge, dir-over-file) using the existing mock `FileSystemPort` pattern in `file_ops.rs`
- [x] 4.2 Unit tests for `ConflictPolicy` propagation (Overwrite All / Skip All resolve subsequent conflicts without prompting)
- [x] 4.3 Unit tests for partial-failure batches (one file fails, others succeed; summary counts are correct)
- [x] 4.4 Unit test for move: skipped directory entries are not deleted from source after a partially-skipped move
- [x] 4.5 Integration test (gated like existing trash/open tests) exercising a real background copy with a real conflict end-to-end

## 5. Verification

- [x] 5.1 `cargo check` / `cargo clippy --all-targets --all-features`
- [x] 5.2 `cargo test`
- [ ] 5.3 Manual verification: copy a directory onto an existing directory with overlapping and non-overlapping files, confirm merge + prompts; cancel a large copy mid-way and confirm partial state matches expectations
