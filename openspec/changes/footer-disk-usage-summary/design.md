## Context

`draw_footer` (`src/presentation/terminal/renderer.rs:432-467`) prints one left-aligned string on the row at `self.rows - FOOTER_ROWS + 1`: either the filter text, the quick-view page position, `"Selected: N"`, or `"{dirs} dirs, {files} files"`. `FileEntry.size` (`src/domain/entry.rs`) is already populated for every entry by `list_dir` (`fs_adapter.rs`). `human_size` (`quick_view.rs:232`) is a private free function, already unit-tested, that formats bytes as e.g. "1.23 MB". The only existing platform-gated code in the repo is `#[cfg(unix)]` blocks in `renderer.rs` around `libc::select` for TTY polling — the same pattern applies here for a new `libc::statvfs` call. `windows-sys`/`windows` are present in `Cargo.lock` only as *transitive* dependencies (via crossterm et al.); using them directly would require adding an explicit `Cargo.toml` entry, and there's no way to test a Windows implementation in this environment.

## Goals / Non-Goals

**Goals:**
- Browsing (no selection): show the free/total space of the volume containing the current directory, right-aligned, matching Total Commander's "{free} of {total} free" footer convention.
- Multi-select active: show the total size of the selected files (not directories), right-aligned — unchanged from the original design.
- Reuse the existing `human_size` formatter — no duplication.
- Degrade gracefully (omit the figure, don't corrupt the line) when disk usage can't be determined or the terminal is too narrow.

**Non-Goals:**
- No recursive directory size ("size on disk" for a folder tree) for the selection case — matches the existing quick-view convention.
- No Windows disk-usage support in this change — flagged as a known gap rather than a blind, untested implementation.
- No caching/invalidation — both figures are recomputed on each footer draw (a `statvfs` syscall, or an O(entries) sum), same cost class as the existing dirs/files count on the same line.
- No change to `FileEntry`, `FileSystemPort`, or `list_dir`.

## Decisions

**New `disk_usage_adapter.rs` in `infrastructure/`, not inline in `renderer.rs`.** Even though the existing TTY-polling `libc` calls live directly in `renderer.rs` (presentation layer), disk-space querying is a distinct, independently-testable concern with its own precedent in this codebase's structure (`fs_adapter.rs`, `favorites_adapter.rs`, etc. — one focused adapter per external concern). `disk_free_and_total(path: &Path) -> Option<(u64, u64)>` returns `(free_bytes, total_bytes)`.

**`#[cfg(unix)]` real implementation via `libc::statvfs`; `#[cfg(not(unix))]` returns `None`.** `libc::statvfs` has identical field names on macOS and Linux (POSIX-standardized: `f_frsize`, `f_blocks`, `f_bavail`), so one code path covers both without a `cfg(target_os = ...)` split. `f_bavail` (blocks available to an unprivileged user) is used for "free" rather than `f_bfree` (includes root-reserved blocks) — this matches what a normal user can actually use, the same semantic Total Commander and `GetDiskFreeSpaceExW` report. Total = `f_blocks * f_frsize`, free = `f_bavail * f_frsize`.

**Windows returns `None` for now (ponytail-marked gap).** Adding `windows-sys` as a real dependency to call `GetDiskFreeSpaceExW` is the correct eventual fix, but it can't be verified without a Windows machine; shipping an unverified `unsafe` FFI call is worse than clearly omitting the figure on that platform. The footer already has a graceful "omit if unavailable" path (needed anyway for the narrow-terminal case), so `None` composes into existing behavior for free.

**Reuse `human_size` by making it `pub(crate)`.** Unchanged from the original design — smallest change, no duplication.

**Right-alignment mechanism unchanged.** `draw_footer` still computes a `(left_text, color, Option<right_text>)` triple and right-aligns `right_text` based on `self.columns`, omitting it if it wouldn't fit — this now covers both the disk-usage string (browsing) and the selection-size string (multi-select) through the same code path.

## Risks / Trade-offs

- **[`statvfs` syscall on every footer redraw while browsing]** → Redraws happen on user input, not a timer; a single `statvfs` call is cheap (no tree walk, unlike a hypothetical recursive directory size) and is the same cost class as the existing dirs/files count.
- **[No Windows support]** → Explicitly documented rather than silently broken; the footer just omits the figure there, same as the "can't be determined" and "too narrow" cases. Revisit if/when Windows testing is available.
- **[`human_size` visibility widened to `pub(crate)`]** → Small, deliberate API surface increase within the crate; no external API is affected (binary crate).
- **[A path with an interior NUL byte can't become a `CString`]** → Handled by returning `None` (`CString::new` failure), exercised directly in a unit test.
