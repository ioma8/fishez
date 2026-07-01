## Context

`draw_footer` (`src/presentation/terminal/renderer.rs`) currently computes the multi-select branch synchronously and non-recursively: `panel.multi_selected.iter().filter_map(...).filter(|e| !e.is_dir()).map(|e| e.size).sum()` — directories contribute nothing. `PanelState.multi_selected` is a `HashSet<usize>` of indices into `entries`. Selection is toggled in `handle_selection` (`shortcuts.rs:219-238`, `Space`/`Esc`), and `navigate::refresh_entries` (`navigate.rs:9`) already clears `multi_selected` on every directory reload — a useful existing invalidation point. The transfer feature (`transfer.rs`) already established the pattern this change reuses: a background `std::thread` reporting through the existing `Message`/`mpsc::channel` drained on the event loop's 100ms poll tick (`event_loop.rs:54`), with `Arc<AtomicBool>` cancellation checked between steps of a walk.

## Goals / Non-Goals

**Goals:**
- Multi-select footer shows `"Selected: {size} of {total}"`, both recursive, matching Total Commander.
- Neither recursive walk (selection or whole-directory) blocks the UI thread.
- The directory total is computed once per directory visit, not once per keystroke.
- A superseded selection-size computation's result is discarded when the selection changes again.

**Non-Goals:**
- No incremental/delta size tracking on toggle (e.g., add/subtract a folder's size when it's added/removed from the selection) — every selection change re-walks the full current selection from scratch. Simpler, and correct; revisit only if reported as slow in practice.
- No persistent cross-session cache of directory sizes.
- No change to the no-selection (browsing) footer — dirs/files count and volume free/total are unaffected.
- No live progress percentage during the walk — just a placeholder (existing count-based text) until the final numbers are ready, matching the "no ETA" simplicity already accepted for the transfer feature's non-goals.

## Decisions

**New `src/application/use_cases/dir_size.rs` with one entrypoint, reused for both totals.** `pub fn total_size(fs_port: &impl FileSystemPort, roots: &[FileEntry], cancel: &AtomicBool) -> u64` sums sizes over `roots`. Both "directory total" (`roots = panel.entries` minus `..`) and "selection total" (`roots` = the selected `FileEntry`s) call this same function.

**Revised after real-world testing: prefer shelling out to `du`, not a pure-Rust walk.** The original plan (recurse via `fs_port.list_dir`, summing per-entry `size`) turned out to be impractically slow on real directories containing many large subtrees — e.g. a directory of ~20 Rust project checkouts (~124 GB, each with a `target/`) never finished in practical time, because the pure-Rust walk allocates a `Vec<FileEntry>` per directory level and pays Rust-side overhead per file. `du` is already installed on every Unix system, is a decades-optimized C loop with none of that overhead, and — same architectural pattern this app already uses for `fd`/`ripgrep` — shelling out to it is both simpler and dramatically faster (the same ~124 GB directory: **~11 seconds via `du`**, versus a walk that hadn't finished after minutes). `total_size` now tries `total_size_via_du` first (`#[cfg(unix)]`) and falls back to the original pure-Rust `total_size_fallback` when `du` isn't available (non-Unix, or missing from `PATH`) — that fallback is kept, tested, and is the only path on non-Unix.

**`du` cancellation is killing the child process, not polling a flag inside a walk.** `total_size_via_du` spawns `du -sk -- <roots>`, drains its stdout on a dedicated reader thread (to avoid a full-pipe deadlock if a large `roots` list produces a lot of output), and polls `cancel`/`child.try_wait()` every 50ms; on cancellation it kills the child instead of waiting for it to finish. This composes with the existing generation-counter staleness mechanism below exactly the same way the old walk's `cancel.load()` checks did.

**Semantic shift: `du`'s numbers are on-disk block usage, not apparent byte size.** `du -sk` reports space actually allocated (rounded up to the filesystem's block size), not the exact sum of file lengths — which is arguably a *better* match for "real size" (Total Commander's own footer reports disk usage, not apparent size) than the original apparent-byte-sum design. This does mean small-file totals are no longer byte-exact; tests that need exact byte counts exercise `total_size_fallback` directly instead of the public `total_size` entrypoint.

**Generation counters, not job handles, for staleness.** `PanelState` gains:
```rust
pub enum SizeFigure { Idle, Computing(u64), Ready(u64) } // u64 = accumulated/generation as appropriate
pub dir_total: SizeFigure,
pub dir_total_generation: u64,
pub selection_total: SizeFigure,
pub selection_total_generation: u64,
pub selection_cancel: Option<Arc<AtomicBool>>,
```
Each time a job is (re-)kicked off, the relevant generation counter is incremented and captured in the spawned closure; the `Message` result carries that generation back. On arrival, the panel compares the message's generation to its current counter — equal means still relevant (apply and store `Ready`), different means stale (silently discard). This avoids needing to track/cancel-join thread handles directly and composes with the existing `Message` dispatch in `event_loop.rs`.

**Directory-total job triggers lazily, not on every directory visit.** Kicked off the first time a selection becomes non-empty in a directory where `dir_total` is `Idle` (freshly reset by `refresh_entries`). Never computed just for browsing (Non-Goal: don't add background CPU work nobody asked for) — matches the ladder's "does this need to exist yet?" check.

**Selection-total job re-fires on every `Space`/`Esc` selection change while count > 0.** Before spawning, if `selection_cancel` is `Some`, set it to `true` (stops the superseded walk early) and bump `selection_total_generation`; set `selection_total` back to `Computing(generation)` so the footer shows the placeholder again immediately. `Esc` (clear selection) doesn't need a job — just resets `selection_total` to `Idle`.

**`Message` gains two variants**, mirroring the transfer feature's event style:
```rust
DirTotalResult { pane: ActivePane, generation: u64, total: u64 },
SelectionTotalResult { pane: ActivePane, generation: u64, total: u64 },
```
Drained in `handle_async_messages` (`event_loop.rs`) the same way `Message::Transfer` already is.

**Footer placeholder: reuse the existing count-based text, then show whichever figure is ready first.** While neither total is `Ready`, `draw_footer`'s multi-select branch shows `"Selected: {count}"` (today's text) unchanged. Once `selection_total` resolves (usually first — it's normally a much smaller subset than the whole directory), it shows `"Selected: {size}"` alone; once `dir_total` also resolves, `" of {total}"` is appended. No reason to hold back a ready number while waiting on the slower one.

**Further speed-up after real-world testing: parallel `du`, plus a per-path cache — Spotlight rejected.** `du` alone (single sequential process) was ~11s on the reported ~124 GB directory, still too slow by the user's judgment. Considered macOS Spotlight (`mdfind`) as a near-instant path; rejected after testing — it found only 76 of 222 real files in a small test project (build artifacts commonly excluded/lagging), and doesn't expose a pre-aggregated size sum anyway, so it would be neither fast nor correct. Instead: (1) `total_size_via_du` now splits `roots` into `available_parallelism()` chunks and runs one `du` process per chunk concurrently (all-or-nothing spawn, falling back entirely to `total_size_fallback` if any chunk fails to spawn) — cut the reported directory to ~5s; (2) `PanelState.dir_total_cache: HashMap<PathBuf, u64>` caches each directory's total for the session, so `refresh_entries` restores `Ready` immediately when revisiting an already-computed path instead of resetting to `Idle` — genuinely instant for the common "browse up, come back down" pattern, at the cost of possible staleness if a directory's contents change between visits (no auto-invalidation; accepted trade-off).

## Risks / Trade-offs

- **[Full re-walk per selection change, no incremental tracking]** → Accepted per Non-Goals; a full walk of even a large selection is bounded by directory-listing I/O, not computation, and cancellation means only the *last* selection change's walk runs to completion.
- **[Directory total requires walking the entire current directory, which can be large]** → Triggered lazily (only once a selection is made), cached for the rest of the visit, and cancellable like the selection job if the user navigates away mid-walk (generation bump makes the eventual result a no-op).
- **[Two more `Message` variants and two more fields per `PanelState`]** → Same shape as the existing `Message::Transfer`/`TransferUiState` addition from the prior change; consistent with the codebase's established pattern rather than a new abstraction.
- **[Navigating away while a directory-total walk is in flight]** → The generation counter is bumped by `refresh_entries` resetting `dir_total` to `Idle` with a new generation; the stale result is discarded on arrival, same mechanism as selection staleness.
- **[`du` might not be on `PATH`, or a future non-Unix build has no equivalent]** → Falls back to the pure-Rust walker automatically; slower, but correct and always available.
- **[Even `du` isn't instantaneous on very large or cold-cache directories]** → Still bounded by real disk I/O, not by our own overhead; ~11s for a ~124 GB, 20-project directory in practice. No cutoff/cap was requested — the number is always the real, eventually-correct total, just slower to appear on truly enormous directories.
