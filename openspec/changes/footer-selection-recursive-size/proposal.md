## Why

Total Commander's footer, when a multi-selection is active, shows "Selected: {selected size} of {total size}" where both sizes are *real* (recursive) — a selected folder's size includes everything inside it, and the denominator is the total size of the whole current directory tree. fishez's current multi-select footer (shipped in `footer-disk-usage-summary`) shows `"Selected: {count}"` on the left and a right-aligned size that only sums selected *files*, ignoring selected directories entirely (`e.is_dir()` filters them out). This undercounts selections that include folders and doesn't match the format the user asked for.

## What Changes

- **BREAKING** (behavior, not API): the multi-select footer text changes from `"Selected: {count}"` (+ separate right-aligned files-only size) to a single string `"Selected: {selected_size} of {dir_total_size}"`, where:
  - `selected_size` is the recursive size of the selection — files contribute their own size, selected directories contribute the recursive size of everything inside them.
  - `dir_total_size` is the recursive size of the *entire current directory* (all entries, all folders walked), independent of what's selected — the same denominator Total Commander shows.
- Recursive directory walks run on a background thread (same architecture as the copy/move transfer feature) so selecting or entering a large directory never freezes the UI; the footer shows the previous count-based text as a placeholder until both sizes are ready.
- The directory total is computed once per directory visit and cached until the listing refreshes (navigation, filter, or any operation that calls `refresh_entries`); the selection total is recomputed on each selection change, with any in-flight computation for a superseded selection cancelled.
- The no-selection (browsing) footer — dirs/files count plus volume free/total space — is unchanged.

## Capabilities

### New Capabilities
- `footer-selection-recursive-size`: background-computed, recursive selected-size and directory-total-size figures shown in the multi-select footer.

### Modified Capabilities
(none — `footer-disk-usage-summary` hasn't been archived to `openspec/specs/` yet, so there's no base spec to delta against; this proposal's new capability spec supersedes that change's still-open "Selected-files size shown during multi-select" requirement in practice.)

## Impact

- `src/application/use_cases/dir_size.rs` (new): cancellable recursive size computation, reused for both the directory total and the selection total. Prefers shelling out to `du` (already installed on Unix; the same "reuse a specialized external tool" pattern as `fd`/`ripgrep`) since a pure-Rust recursive walk proved impractically slow on real large directories during manual verification (~124 GB across ~20 project checkouts never finished; `du` resolves the same directory in ~11s). Falls back to a pure-Rust walk over `FileSystemPort` when `du` is unavailable (non-Unix, or missing from `PATH`).
- `src/application/state.rs` (`PanelState`): gains cached/in-flight state for both totals plus generation counters to discard stale background results.
- `src/presentation/input_handler.rs` / `shortcuts.rs`: kick off background size jobs on selection change (`Space`, `Esc`) and on directory refresh, reusing the existing `Message`/`mpsc` channel and background-thread pattern from the transfer feature.
- `src/presentation/event_loop.rs`: new `Message` variants for size-job results, drained on the existing poll tick.
- `src/presentation/terminal/renderer.rs` (`draw_footer`): multi-select branch reformatted to `"Selected: {size} of {total}"`; no other branch changes.
- No new dependencies — reuses `std::thread`/`std::sync::mpsc`/`Arc<AtomicBool>` already used by the transfer feature, and `FileSystemPort::list_dir` already used everywhere.
