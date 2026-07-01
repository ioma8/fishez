## Why

Total Commander always shows a size summary in the panel footer — free/total space of the current volume while browsing, or the total of the selected files during multi-select — so users know how much room they have and how much they're about to copy/move/delete without opening a separate dialog. fishez's footer (`draw_footer`, `src/presentation/terminal/renderer.rs:432-467`) currently only shows counts ("3 dirs, 12 files" or "Selected: 4"), never size or disk space.

## What Changes

- The footer row that currently shows "N dirs, M files" or "Selected: N" gains a right-aligned figure on the same row.
- Browsing (nothing selected): right side shows the free/total space of the volume containing the current directory, in the form "{free} of {total} free" (e.g. "76.50 GB of 117.10 GB free"), matching Total Commander's convention.
- Multi-select active: right side shows the total size of the selected files (not directories) — this part is unchanged from the original proposal and already implemented.
- Disk free/total is queried via `statvfs` on Unix (macOS/Linux); on other platforms it's simply omitted (**no Windows implementation** — would require adding `windows-sys` as a new direct dependency and calling `GetDiskFreeSpaceExW`, which is untestable in this environment; left as a follow-up).
- On terminals too narrow to fit both the left-hand text and the right-hand figure, the figure is dropped rather than overlapping/wrapping.

## Capabilities

### New Capabilities
- `footer-size-summary`: right-aligned disk free/total (browsing) or selected-file-size total (multi-select) rendered on the panel footer's counts row.

### Modified Capabilities
(none — no existing `openspec/specs/` capabilities yet)

## Impact

- `src/presentation/terminal/renderer.rs:432-467` (`draw_footer`): computes and right-aligns the disk-usage or selection-size string alongside the existing left-hand text.
- `src/infrastructure/disk_usage_adapter.rs` (new): `disk_free_and_total(path) -> Option<(u64, u64)>` via `libc::statvfs` on Unix; `None` on other platforms.
- `src/application/use_cases/quick_view.rs`: its private `human_size` helper is reused (made `pub(crate)`) rather than duplicated.
- No new dependencies (Unix path uses the already-present `libc` crate); no change to `FileEntry`/`FileSystemPort`.
