## Context

`copy_items`/`move_items` (`src/application/use_cases/file_copy.rs`, `file_move.rs`) are called synchronously from `handle_copy_dest_input`/`handle_move_dest_input` (`src/presentation/input_handler.rs:949-1018`) on Enter. They walk directories recursively via plain `std::fs` calls with no progress callback and no existence check before `copy_file`/`rename`. The event loop (`event_loop.rs:54`) already polls for terminal events every 100ms in a plain loop — that same tick is the natural place to also drain a progress channel, no new async runtime needed. No async runtime (tokio, etc.) is used anywhere in the codebase; `std::thread` + `std::sync::mpsc` is the smallest tool that fits the existing single-threaded, poll-driven architecture.

## Goals / Non-Goals

**Goals:**
- Copy/move never blocks the UI thread for more than a single small file's worth of I/O.
- No existing destination file is ever overwritten without an explicit user choice.
- Directory-onto-directory copy/move merges instead of silently replacing.
- Per-file failures don't abort the whole batch.

**Non-Goals:**
- No pause/resume across app restarts, no persisted transfer queue.
- No rollback of already-copied files on cancel or failure (matches existing "just get the safe default, no undo machinery" scope).
- No throughput/speed/ETA display — file-count + current-name is enough for the TC-parity ask; add later if requested.
- No parallel multi-file transfer (single background worker thread processes items sequentially) — simplicity over throughput; revisit only if users report large batches feeling slow.

## Decisions

**Threading model: one worker thread + `mpsc` channel, no async runtime.**
`std::thread::spawn` runs the whole batch; it sends `TransferEvent` values (`Progress { current: PathBuf, done: usize, total: usize }`, `Conflict { path: PathBuf, reply: Sender<ConflictResolution> }`, `Done { ok: usize, failed: Vec<(PathBuf, String)> }`) over an `mpsc::channel`. The event loop's existing 100ms poll tick does a non-blocking `try_recv` loop after the terminal-event poll and updates `AppState` / triggers a redraw. This reuses the existing loop instead of adding a second thread of control (no tokio, no extra dependency — rung 4 of the ladder: nothing already installed solves this, but a channel is one line to add and stdlib covers it).

**Conflict resolution: synchronous back-channel per conflict.**
When the worker hits an existing destination path, it sends `Conflict` with a one-shot `mpsc::Sender<ConflictResolution>` and blocks on `recv()` waiting for the reply. The main thread shows the conflict prompt (modeled on `draw_delete_prompt`, `overlays.rs:33`) and sends the user's choice back down that sender when they press a key. This keeps the worker thread's logic simple (blocking wait, no polling) while the UI thread stays responsive (it only blocks briefly synchronously sending a reply after a keypress, not waiting on I/O).

**"Overwrite All" / "Skip All" state lives in the worker, not round-tripped every time.**
The worker holds a `ConflictPolicy` (`AskEachTime | AlwaysOverwrite | AlwaysSkip`) that starts at `AskEachTime` and gets updated in place when the user picks "All" — avoiding a prompt per file for the common bulk-overwrite case.

**Pre-count total via a cheap upfront walk.**
Before starting the transfer, the worker walks the source tree once (metadata-only, no reads) to compute `total` item count, then starts the actual copy. For directories this is a second traversal, but it's stat-only and negligible next to the copy itself; without it "N of M" can't be shown for directory copies, and Non-Goals already rule out a fancier lazy-count scheme.

**Directory merge semantics.**
`copy_one`/`move_one` change: if `dest` exists and is a directory, do not `create_dir_all` (currently unconditional) — just recurse into it and apply the same per-entry conflict logic to children. If `dest` exists and is a file, that's a conflict. `move_one`'s cross-device fallback (`file_move.rs:26-39`) reuses `copy_items` then removes the source — for directories, only remove source entries that were actually resolved (not skipped), so a partially-skipped move doesn't delete skipped files' source copies.

**Cancellation is cooperative, checked between items.**
`Esc` sends a `Cancel` message on a second small control channel (or an `AtomicBool` flag) that the worker checks between items (and between recursion steps for a big directory), not mid-`copy_file`. Simpler than interrupting an in-flight syscall, and finishing the current file before stopping matches user expectation ("it stopped after the file it was on").

**`FileSystemPort` unchanged.**
`copy_file`/`rename`/`create_dir` keep their current signatures; only `file_copy::copy_items`/`file_move::move_items` change their calling convention (return a job handle / take a channel instead of returning `Result<(), String>` synchronously). Existing unit tests for the port mocks in `file_ops.rs` are unaffected.

## Risks / Trade-offs

- **[Sequential transfer may feel slow for many small files]** → Acceptable per Non-Goals; parallelizing is a follow-up if it's actually reported as slow, not a preemptive optimization.
- **[Upfront pre-count walk adds a second directory traversal]** → Stat-only, no file reads; negligible versus the transfer itself. If a directory changes between pre-count and transfer (race), `total` may be slightly off — acceptable, it's a progress hint, not a correctness guarantee.
- **[Blocking `recv()` in the worker while waiting on a conflict prompt]** → Intentional; the worker thread is meant to be idle during user decision time, this isn't a busy-wait.
- **[Partial move on cancel/skip leaves source and destination both containing some files]** → Documented as expected (Non-Goals: no rollback); the final summary must clearly state what succeeded so the user isn't surprised.
- **[Existing callers of `copy_items`/`move_items` in `input_handler.rs` must switch from synchronous `match ... { Ok/Err }` to spawning + storing a handle in state]** → Contained to two call sites; covered explicitly in tasks.

## Migration Plan

No data migration. Rollout is a single code change; land it as one PR since splitting worker/UI halves would leave the tool in a half-wired, untestable state. No feature flag — this is a bug-safety fix (silent overwrite) as much as a UX one, so it should apply unconditionally once merged.

## Open Questions

- None — behavior for all identified edge cases (skip, overwrite, overwrite-all, skip-all, cancel, directory merge, per-file failure) is specified above.
