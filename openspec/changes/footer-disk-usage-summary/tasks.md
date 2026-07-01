## 1. Expose the size formatter

- [x] 1.1 Change `human_size` in `quick_view.rs` from private to `pub(crate)` (no logic change)

## 2. Disk usage adapter

- [x] 2.1 Add `src/infrastructure/disk_usage_adapter.rs` with `disk_free_and_total(path: &Path) -> Option<(u64, u64)>`: `#[cfg(unix)]` real implementation via `libc::statvfs`, `#[cfg(not(unix))]` returns `None`
- [x] 2.2 Register the module and re-export in `src/infrastructure/mod.rs`

## 3. Compute and render the right-aligned figure

- [x] 3.1 In `draw_footer` (`renderer.rs:432-467`), browsing case: call `disk_free_and_total(&panel.current_path)` and format as `"{free} of {total} free"` using `human_size`
- [x] 3.2 Multi-select case: keep the existing total size of selected files (not directories)
- [x] 3.3 Right-align the applicable figure on the same row as the existing left-hand text, based on `self.columns`
- [x] 3.4 When left text + figure would not fit within `self.columns`, or disk usage can't be determined, omit the figure entirely instead of truncating or wrapping
- [x] 3.5 Leave the `Filter:` and quick-view "File N / M" footer branches unchanged (no size/disk figure makes sense there)

## 4. Tests

- [x] 4.1 Unit test: browsing a directory shows a disk free/total figure ("... of ... free") alongside the unaffected dirs/files count
- [x] 4.2 Unit test: when disk usage can't be determined (e.g. a path with an interior NUL byte), the figure is omitted
- [x] 4.3 Unit test: multi-selecting files shows the selected total instead of the disk figure
- [x] 4.4 Unit test: multi-selecting a mix of files and directories excludes directory sizes from the selected total
- [x] 4.5 Unit test: narrow terminal width causes the figure to be omitted without corrupting the left-hand text
- [x] 4.6 Unit tests for `disk_free_and_total` itself: succeeds for a real path (`/`), returns `None` for a path with an interior NUL byte

## 5. Verification

- [x] 5.1 `cargo check` / `cargo clippy --all-targets --all-features`
- [x] 5.2 `cargo test`
- [ ] 5.3 Manual verification: browse a directory and confirm the free/total figure matches `df`/Finder/Explorer for that volume; multi-select some files and confirm it switches to the selected total
