# Implementation Plan 3 — Performance & Fluency

Goal: quick view appears on the same frame as the keypress for normal files; all async
results land without artificial delay; redraws are one syscall. Items are ordered —
implement top to bottom, each is independently shippable.

## 1. Kill the 100ms async-result delay

**Problem:** `run()` blocks in `event::poll(100ms)` (`event_loop.rs:73`), and
`handle_async_messages` (`event_loop.rs:781`) runs only after the poll returns and drains
**one** message per iteration. Every async result (quick view, search, transfer progress,
dir sizes) waits up to 100ms in the channel.

**Change (`event_loop.rs`):**
- Poll timeout `100ms` → `15ms`: `event::poll(std::time::Duration::from_millis(15))`.
- In `handle_async_messages`, change `if let Ok(message) = receiver.try_recv()` to
  `while let Ok(message) = receiver.try_recv()`. Each arm already redraws; that is
  acceptable (bursts are rare) — do NOT restructure the arms.
- Onboarding auto-dismiss at `event_loop.rs:53` uses elapsed time, unaffected.

**Test:** none practical (loop timing). Rely on existing suite passing.

## 2. Synchronous fast path for small text quick view

**Problem:** `schedule_quick_view` (`input_handler.rs:243`) always sets
`QuickViewMode::Loading` + spawns a thread → spinner flash even for tiny files. Also hit
on Left/Right file flipping (`event_loop.rs:539`).

**Change (`input_handler.rs`, in `schedule_quick_view`):**
```rust
const SYNC_PREVIEW_MAX: u64 = 1024 * 1024; // 1 MB

if let Some(path) = panel.get_selected_path() {
    let small = std::fs::metadata(&path)
        .map(|m| m.is_file() && m.len() <= SYNC_PREVIEW_MAX)
        .unwrap_or(false);
    let is_image = quick_view::looks_like_image(&path); // new helper, see below
    if small && !is_image {
        panel.mode = PanelMode::QuickView(quick_view::preview(path, columns));
        return;
    }
    // existing Loading + thread::spawn path unchanged
}
```
- Add `pub fn looks_like_image(path: &Path) -> bool` in `quick_view.rs`: returns true for
  `raw_image::supports_path(path)` or an image extension (reuse `classify_extension`).
  Images stay async (decode can be slow even under 1MB).
- Directories: `metadata.is_file()` is false → stay async (network mounts can hang).
  Local dirs are fast but the thread path is now only ~15ms behind (item 1). Fine.
- Caller draws after input handling already, so the panel mode set here renders
  immediately — verify no extra draw needed (mirror what the `QuickViewResult` arm does).

**Test:** unit test in `input_handler.rs`: small temp .txt file, call
`schedule_quick_view`, assert `panel.mode` is `QuickView(Text {..})` immediately (no
Loading), and that nothing arrives on the channel.

## 3. Cap text preview size

**Problem:** `preview_text` (`quick_view.rs:79`) reads, wraps, and highlights the whole
file — O(file size), only a screenful is shown.

**Change (`quick_view.rs`):**
```rust
const TEXT_PREVIEW_MAX_BYTES: u64 = 1024 * 1024; // 1 MB
const TEXT_PREVIEW_MAX_LINES: usize = 5_000;
```
- Replace `fs::read_to_string(path)` with: open file, `take(TEXT_PREVIEW_MAX_BYTES)`,
  read to `Vec<u8>`, `String::from_utf8_lossy`. If the file was larger (compare
  `metadata.len()`), trim the last (possibly split) line.
- After wrapping, `lines.truncate(TEXT_PREVIEW_MAX_LINES)`.
- If either cap hit, push a final line: `"… preview truncated"`.
- `highlight()` now runs on capped input — no separate change needed.

**Test:** write a 2MB file of `"word "` repeats, assert preview returns, last line is the
truncation marker, and `lines.len() <= TEXT_PREVIEW_MAX_LINES + 1`.

## 4. Replace tree_magic_mini with extension list + NUL sniff

**Problem:** `get_type` (`quick_view.rs:60`) whitelist covers only txt/md/rs/toml; every
other file hits `tree_magic_mini::from_filepath` (loads system MIME magic DB, reads file).

**Change (`quick_view.rs`):**
- Expand `classify_extension` text arm:
  `txt md markdown rs toml json yaml yml js ts jsx tsx py rb go c h cpp hpp java kt swift
  sh bash zsh fish css scss html xml svg sql lock cfg conf ini env gitignore log csv tsv`
  (svg is not in the image arm today; classify it as Text — markup preview).
- Replace the `tree_magic_mini` fallback with:
```rust
fn sniff_is_text(path: &Path) -> bool {
    let mut buf = [0u8; 8192];
    let Ok(mut f) = fs::File::open(path) else { return false };
    let Ok(n) = f.read(&mut buf) else { return false };
    n > 0 && !buf[..n].contains(&0)
}
```
  `get_type` fallback order: known extension → `sniff_is_text` → `FileType::Other`.
  Extensionless files (Makefile, LICENSE) now preview via the sniff.
- Remove `tree_magic_mini` from `Cargo.toml` and the `use`.

**Test:** existing `test_get_type_*` tests must pass unchanged (data.bin test writes
bytes without NUL — `[0u8, ...]` starts with NUL, still passes). Add: extensionless file
with plain ASCII → Text; file starting with `\0\0` → Other.

## 5. Decode images once in preview_image

**Problem:** `preview_image` (`quick_view.rs:179`) decodes up to 3×: `extract_thumbnail`,
`load_from_memory(&buf)` (existence check), then `downscale_image_if_needed` decodes the
same bytes again.

**Change (`quick_view.rs`):**
```rust
fn preview_image(path: &Path, wrap_width: u16) -> QuickViewMode {
    if let Some(raw_bytes) = raw_image::try_render_from_raw(path) {
        return QuickViewMode::Image(raw_bytes);
    }
    let Ok(buf) = fs::read(path) else { return QuickViewMode::NotSupported };
    let Some(img) = extract_thumbnail(path).or_else(|| load_from_memory(&buf).ok()) else {
        return QuickViewMode::NotSupported;
    };
    let target = terminal_pixel_limit(wrap_width);
    if img.width() <= target && img.height() <= target {
        return QuickViewMode::Image(buf);
    }
    let resized = imageops::thumbnail(&img, target, target);
    let mut output = Vec::new();
    match JpegEncoder::new_with_quality(&mut output, 80).encode_image(&resized) {
        Ok(_) => QuickViewMode::Image(output),
        Err(_) => QuickViewMode::Image(buf),
    }
}
```
- Delete `downscale_image_if_needed`; drop the `.to_rgb8()` existence check.
- Note the thumbnail branch changes behavior slightly: if EXIF thumbnail exists it is
  used for dimension check AND as resize source — acceptable, it was already preferred.

**Test:** existing `preview_image_downscales_large_images` must still pass.

## 6. Buffer stdout writes

**Problem:** renderer `queue!`s into raw `std::io::Stdout` (`renderer.rs:119`) — a full
redraw is many small write syscalls.

**Change (`renderer.rs`):**
- `StdoutKind::Real(std::io::Stdout)` → `Real(std::io::BufWriter<std::io::Stdout>)`.
- Construction: `StdoutKind::Real(std::io::BufWriter::with_capacity(256 * 1024, std::io::stdout()))`.
- `Write` impl arms unchanged (`BufWriter` implements `Write`); `flush()` already called
  at end of every draw (`renderer.rs:166,193`) — audit that **every** public draw entry
  point ends with `self.stdout.flush()`; add where missing (grep `fn draw`).
- `reset_terminal` (`renderer.rs:144`) uses `execute!(std::io::stdout(), ...)` directly
  for mouse capture — leave those, they self-flush; keep the final `self.stdout.flush()`.

**Test:** existing renderer tests use `TestWriter`, unaffected.

## 7. Generation counter for quick view results (do with item 2)

**Problem:** rapid Left/Right spawns parallel preview threads; a slow older result can
overwrite a newer one.

**Change:**
- `PanelState`: add `pub quick_view_generation: u64` (init 0 in `new()`).
- `schedule_quick_view`: `panel.quick_view_generation += 1;` capture the value, include
  it in `Message::QuickViewResult { pane, generation, mode }` (extend the enum).
- Sync fast path (item 2) also bumps the generation, so an in-flight async result from a
  previous file is discarded.
- `handle_async_messages` `QuickViewResult` arm: apply only if
  `generation == panel.quick_view_generation`, else drop silently.

**Test:** unit test: bump generation after send, deliver stale message via the handler
path, assert panel mode unchanged.

## Explicitly out of scope

ratatui migration / diffed rendering, syntect highlighting, async runtime for the UI,
`list_dir` stat batching. Revisit only if the above measurably falls short.
