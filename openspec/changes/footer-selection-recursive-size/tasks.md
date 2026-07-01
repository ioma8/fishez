## 1. Recursive size walker

- [x] 1.1 Add `src/application/use_cases/dir_size.rs` with `total_size(fs_port: &impl FileSystemPort, roots: &[PathBuf], cancel: &AtomicBool) -> u64`, recursing into directories via `fs_port.list_dir`
- [x] 1.2 Check `cancel` between each `list_dir` call so a stopped walk exits early without finishing
- [x] 1.3 Unit tests: file-only roots sum correctly; a directory root sums its recursive contents; a cancelled walk returns early (partial/zero, not a panic or hang)

## 2. Panel state for cached/in-flight totals

- [x] 2.1 Add `SizeFigure` enum (`Idle`, `Computing(u64)`, `Ready(u64)`) and `dir_total`/`dir_total_generation`/`selection_total`/`selection_total_generation`/`selection_cancel` fields to `PanelState`
- [x] 2.2 In `navigate::refresh_entries`, reset `dir_total` to `Idle` and bump `dir_total_generation` alongside the existing `multi_selected.clear()`

## 3. Background jobs wired into selection handling

- [x] 3.1 In `handle_selection` (`shortcuts.rs`), on `Space` toggling selection to non-empty: if `dir_total` is `Idle`, spawn a directory-total job; always spawn (or re-spawn, cancelling any prior in-flight job) a selection-total job
- [x] 3.2 On `Esc` (clear selection), reset `selection_total` to `Idle` and cancel any in-flight selection-total job; no job spawned (folded into `PanelState::clear_multi_selection`, so this also covers navigation/refresh and post-move selection clearing)
- [x] 3.3 Spawning sets the relevant `*_generation` and `SizeFigure::Computing(generation)` immediately so the footer can show the placeholder without waiting for the thread to start
- [x] 3.4 Both jobs run on a background thread via `std::thread::spawn`, sending results back through the existing `Sender<Message>`

## 4. Message wiring

- [x] 4.1 Add `Message::DirTotalResult { pane, generation, total }` and `Message::SelectionTotalResult { pane, generation, total }`
- [x] 4.2 In `handle_async_messages` (`event_loop.rs`), on each result: compare the message's generation to the panel's current generation; apply (`Ready(total)`) only if it matches, otherwise discard silently (extracted as `apply_dir_total_result`/`apply_selection_total_result` for direct unit testing)
- [x] 4.3 Trigger a redraw after applying a result so the footer updates once both totals are ready

## 5. Footer rendering

- [x] 5.1 In `draw_footer`'s multi-select branch: if both `dir_total` and `selection_total` are `Ready`, render `"Selected: {human_size(selection)} of {human_size(dir_total)}"`; otherwise keep today's `"Selected: {count}"` text
- [x] 5.2 Remove the old files-only right-aligned selected-size figure (superseded by the unified string)
- [x] 5.3 Leave the no-selection (browsing) branch — dirs/files count plus volume free/total — unchanged

## 6. Tests

- [x] 6.1 Unit test: selecting only files shows `"Selected: {size} of {total}"` once both totals resolve, with `{size}` equal to the sum of selected file sizes
- [x] 6.2 Unit test: selecting a directory includes its recursive contents in `{size}`, not just the directory's own (zero) size (covered at the `dir_size` walker level: `mixes_files_and_directories_in_the_same_root_set`, `sums_recursive_contents_of_a_directory_root`)
- [x] 6.3 Unit test: `{total}` reflects the whole current directory regardless of which subset is selected (footer formatting test supplies independent `Ready` values for selection vs. dir total, proving they're separate figures)
- [x] 6.4 Unit test: before totals resolve (simulate `Computing`/`Idle` state), the footer still shows the count-based placeholder
- [x] 6.5 Unit test: re-selecting before a prior computation resolves discards the stale generation's result when it arrives
- [x] 6.6 Unit test: navigating to a new directory resets `dir_total` to `Idle` and invalidates any in-flight generation

## 7. Verification

- [x] 7.1 `cargo check` / `cargo clippy --all-targets --all-features`
- [x] 7.2 `cargo test`
- [ ] 7.3 Manual verification: select files and folders of known size in a real directory, confirm the recursive total matches `du -sh`; select/deselect rapidly and confirm no stale numbers flash; navigate away mid-selection and confirm no crash or stuck placeholder

## 8. Performance fix: shell out to `du` instead of a pure-Rust walk

Found during manual verification: on a real directory containing ~20 large project checkouts (~124 GB), the pure-Rust `total_size` walk from task 1 never finished in practical time, so the footer stayed stuck on the count-based placeholder indefinitely.

- [x] 8.1 Add `total_size_via_du` (`#[cfg(unix)]`): spawn `du -sk -- <roots>`, drain stdout on a reader thread (avoids pipe deadlock), poll `cancel`/`try_wait()` and kill the child on cancellation
- [x] 8.2 `total_size` tries the `du` path first, falling back to the original pure-Rust walk (renamed `total_size_fallback`) when `du` is unavailable, fails to spawn, or on non-Unix
- [x] 8.3 Update tests: byte-exact assertions moved to exercise `total_size_fallback` directly (`du` reports block-rounded disk usage, not exact byte sums); added tests for the public `total_size` entrypoint (at-least-apparent-size, empty roots, up-front cancellation)
- [x] 8.4 Verified against the real reported directory (`/Users/jakubkolcar/projects/customs`, ~124 GB): resolves in ~11s via `du`, versus not finishing within minutes via the old pure-Rust walk
- [x] 8.5 `cargo check` / `cargo clippy --all-targets --all-features` / `cargo test` / `cargo fmt` all clean
