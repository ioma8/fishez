//! Quick view use case - file preview.

use crate::application::use_cases::raw_image;
use crate::application::{PanelMode, PanelState, QuickViewMode};
use image::{DynamicImage, codecs::jpeg::JpegEncoder, imageops, load_from_memory};
use little_exif::exif_tag::ExifTag;
use little_exif::metadata::Metadata;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

const MIN_WRAP_WIDTH: u16 = 20;
const DIR_PREVIEW_LIMIT: usize = 20;
const TEXT_PREVIEW_MAX_BYTES: u64 = 1024 * 1024;
const TEXT_PREVIEW_MAX_LINES: usize = 5_000;

pub fn scroll(panel: &mut PanelState, direction: isize, rows: u16, header: u16, footer: u16) {
    if let PanelMode::QuickView(QuickViewMode::Text { lines, start }) = &mut panel.mode {
        let visible = rows.saturating_sub(header + footer) as usize;
        let max = lines.len().saturating_sub(visible.max(1));
        *start = (*start as isize + direction).clamp(0, max as isize) as usize;
    }
}

pub fn preview(path: PathBuf, wrap_width: u16) -> QuickViewMode {
    if path.is_dir() {
        return preview_directory(&path);
    }
    match get_type(&path) {
        FileType::Text => preview_text(&path, wrap_width),
        FileType::Image => preview_image(&path, wrap_width),
        FileType::Other => QuickViewMode::NotSupported,
    }
}

enum FileType {
    Text,
    Image,
    Other,
}

fn get_type(path: &Path) -> FileType {
    if raw_image::supports_path(path) {
        return FileType::Image;
    }

    let extension = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase());
    let extension_type = classify_extension(extension.as_deref());

    if matches!(extension_type, FileType::Image | FileType::Text) {
        return extension_type;
    }

    if sniff_is_text(path) {
        FileType::Text
    } else {
        FileType::Other
    }
}

pub fn looks_like_image(path: &Path) -> bool {
    if raw_image::supports_path(path) {
        return true;
    }
    let extension = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase());
    matches!(classify_extension(extension.as_deref()), FileType::Image)
}

fn sniff_is_text(path: &Path) -> bool {
    let mut buf = [0u8; 8192];
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let Ok(n) = file.read(&mut buf) else {
        return false;
    };
    n > 0 && !buf[..n].contains(&0)
}

fn classify_extension(extension: Option<&str>) -> FileType {
    match extension {
        Some(
            "txt" | "md" | "markdown" | "rs" | "toml" | "json" | "yaml" | "yml" | "js" | "ts"
            | "jsx" | "tsx" | "py" | "rb" | "go" | "c" | "h" | "cpp" | "hpp" | "java" | "kt"
            | "swift" | "sh" | "bash" | "zsh" | "fish" | "css" | "scss" | "html" | "xml" | "svg"
            | "sql" | "lock" | "cfg" | "conf" | "ini" | "env" | "gitignore" | "log" | "csv" | "tsv",
        ) => FileType::Text,
        Some("png") | Some("jpg") | Some("jpeg") | Some("gif") => FileType::Image,
        _ => FileType::Other,
    }
}

fn preview_text(path: &Path, wrap_width: u16) -> QuickViewMode {
    if let Some(lines) = preview_text_with_bat(path, wrap_width) {
        return QuickViewMode::Text { lines, start: 0 };
    }

    let Ok(metadata) = fs::metadata(path) else {
        return QuickViewMode::NotSupported;
    };
    let Ok(file) = fs::File::open(path) else {
        return QuickViewMode::NotSupported;
    };

    let mut buf = Vec::new();
    if file
        .take(TEXT_PREVIEW_MAX_BYTES)
        .read_to_end(&mut buf)
        .is_err()
    {
        return QuickViewMode::NotSupported;
    }

    let mut capped = metadata.len() > TEXT_PREVIEW_MAX_BYTES;
    let mut content = String::from_utf8_lossy(&buf).into_owned();
    if capped && let Some(last_newline) = content.rfind('\n') {
        content.truncate(last_newline);
    }

    let width = wrap_width.saturating_sub(4).max(MIN_WRAP_WIDTH) as usize;
    let mut lines: Vec<String> = textwrap::wrap(&content, width)
        .into_iter()
        .map(|l| l.to_string())
        .collect();
    if lines.len() > TEXT_PREVIEW_MAX_LINES {
        lines.truncate(TEXT_PREVIEW_MAX_LINES);
        capped = true;
    }
    if capped {
        lines.push("… preview truncated".to_string());
    }
    QuickViewMode::Text { lines, start: 0 }
}

fn preview_text_with_bat(path: &Path, wrap_width: u16) -> Option<Vec<String>> {
    if fs::metadata(path).ok()?.len() > TEXT_PREVIEW_MAX_BYTES {
        return None;
    }
    let output = Command::new("bat")
        .args([
            "--color=always",
            "--plain",
            "--paging=never",
            "--wrap=character",
            "--terminal-width",
            &wrap_width.to_string(),
        ])
        .arg(path)
        .output()
        .ok()?;
    (output.status.success() && !output.stdout.is_empty())
        .then(|| bat_lines_from_output(&output.stdout))
}

fn bat_lines_from_output(output: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(output)
        .lines()
        .take(TEXT_PREVIEW_MAX_LINES)
        .map(String::from)
        .collect()
}

fn preview_directory(path: &Path) -> QuickViewMode {
    let (mut files, mut dirs, mut size, mut total) = (vec![], vec![], 0u64, 0usize);
    if let Ok(entries) = fs::read_dir(path) {
        for e in entries.flatten() {
            total += 1;
            let name = e.file_name().into_string().unwrap_or_default();
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push(name);
            } else {
                size += e.metadata().map(|m| m.len()).unwrap_or(0);
                files.push(name);
            }
        }
    }
    dirs.sort();
    files.sort();
    let mut lines = vec![
        format!("Directory: {}", path.display()),
        format!(
            "Entries: {} (dirs: {}, files: {})",
            total,
            dirs.len(),
            files.len()
        ),
        format!("Size (files only): {}", human_size(size)),
        String::new(),
    ];
    if !dirs.is_empty() {
        lines.push("Directories:".into());
        lines.extend(
            dirs.iter()
                .take(DIR_PREVIEW_LIMIT)
                .map(|d| format!("{}/", d)),
        );
        if dirs.len() > DIR_PREVIEW_LIMIT {
            lines.push(format!("... and {} more", dirs.len() - DIR_PREVIEW_LIMIT));
        }
        lines.push(String::new());
    }
    if !files.is_empty() {
        lines.push("Files:".into());
        lines.extend(files.iter().take(DIR_PREVIEW_LIMIT).cloned());
        if files.len() > DIR_PREVIEW_LIMIT {
            lines.push(format!("... and {} more", files.len() - DIR_PREVIEW_LIMIT));
        }
    }
    QuickViewMode::Directory { lines }
}

fn preview_image(path: &Path, wrap_width: u16) -> QuickViewMode {
    if let Some(raw_bytes) = raw_image::try_render_from_raw(path) {
        return QuickViewMode::Image(raw_bytes);
    }
    let Ok(buf) = fs::read(path) else {
        return QuickViewMode::NotSupported;
    };
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

fn terminal_pixel_limit(wrap_width: u16) -> u32 {
    const PIXELS_PER_COLUMN: u32 = 8;
    const MAX_SIDE: u32 = 2048;
    let side = wrap_width as u32 * PIXELS_PER_COLUMN;
    side.min(MAX_SIDE)
}

fn extract_thumbnail(path: &Path) -> Option<DynamicImage> {
    let meta = Metadata::new_from_path(path).ok()?;
    let tag = meta
        .get_tag(&ExifTag::ThumbnailOffset(vec![], vec![]))
        .next()?;
    let data = if let ExifTag::ThumbnailOffset(_, d) = tag {
        d
    } else {
        return None;
    };
    load_from_memory(data).ok()
}

pub(crate) fn human_size(bytes: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let (mut s, mut i) = (bytes as f64, 0);
    while s >= 1024.0 && i < U.len() - 1 {
        s /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", bytes, U[i])
    } else {
        format!("{:.2} {}", s, U[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::create_temp_dir;
    use std::fs;

    #[test]
    fn test_scroll_clamps_bounds() {
        let mut panel = PanelState::new();
        panel.mode = PanelMode::QuickView(QuickViewMode::Text {
            lines: vec!["a".to_string(); 50],
            start: 0,
        });
        scroll(&mut panel, 100, 10, 1, 1);
        if let PanelMode::QuickView(QuickViewMode::Text { start, .. }) = panel.mode {
            assert!(start <= 48);
        } else {
            panic!("Expected QuickView Text mode");
        }
    }

    #[test]
    fn test_preview_text_returns_text_mode() {
        let base = create_temp_dir("quick_view_text");
        let file_path = base.join("note.txt");
        fs::write(&file_path, "hello world").unwrap();

        let mode = preview_text(&file_path, 10);
        assert!(matches!(mode, QuickViewMode::Text { lines: _, start: _ }));
    }

    #[test]
    fn test_preview_text_uses_min_wrap_width() {
        let base = create_temp_dir("quick_view_wrap");
        let file_path = base.join("long.txt");
        fs::write(&file_path, "word ".repeat(100)).unwrap();

        let mode = preview_text(&file_path, 5);
        if let QuickViewMode::Text { lines, .. } = mode {
            assert!(lines.iter().any(|line| line.contains("word")));
        } else {
            panic!("Expected QuickView Text mode");
        }
    }

    #[test]
    fn test_show_directory_preview_limit() {
        let base = create_temp_dir("quick_view_dir");
        for i in 0..25 {
            let file_path = base.join(format!("file{}.txt", i));
            fs::write(&file_path, "data").unwrap();
        }

        let mode = preview_directory(&base);
        if let QuickViewMode::Directory { lines } = mode {
            let has_more = lines.iter().any(|line| line.contains("... and"));
            assert!(has_more);
        } else {
            panic!("Expected QuickView Directory mode");
        }
    }

    #[test]
    fn test_human_size_units() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1024), "1.00 KB");
        assert_eq!(human_size(1024 * 1024), "1.00 MB");
    }

    #[test]
    fn test_get_type_text_extension() {
        let base = create_temp_dir("quick_view_type_text");
        let file_path = base.join("main.rs");
        fs::write(&file_path, "fn main() {}\n").unwrap();
        assert!(matches!(get_type(&file_path), FileType::Text));
    }

    #[test]
    fn test_get_type_image_extension() {
        let base = create_temp_dir("quick_view_type_image");
        let file_path = base.join("image.gif");
        fs::write(&file_path, b"GIF89a").unwrap();
        assert!(matches!(get_type(&file_path), FileType::Image));
    }

    #[test]
    fn test_get_type_other_extension() {
        let base = create_temp_dir("quick_view_type_other");
        let file_path = base.join("data.bin");
        fs::write(&file_path, [0u8, 159, 146, 150]).unwrap();
        assert!(matches!(get_type(&file_path), FileType::Other));
    }

    #[test]
    fn test_get_type_extensionless_ascii_is_text() {
        let base = create_temp_dir("quick_view_type_extensionless");
        let file_path = base.join("Makefile");
        fs::write(&file_path, "all:\n\tcargo test\n").unwrap();

        assert!(matches!(get_type(&file_path), FileType::Text));
    }

    #[test]
    fn test_get_type_nul_prefixed_file_is_other() {
        let base = create_temp_dir("quick_view_type_nul");
        let file_path = base.join("blob");
        fs::write(&file_path, [0u8, 0u8, b'a']).unwrap();

        assert!(matches!(get_type(&file_path), FileType::Other));
    }

    #[test]
    fn test_preview_text_caps_large_files() {
        let base = create_temp_dir("quick_view_large_text");
        let file_path = base.join("large.txt");
        fs::write(&file_path, "word ".repeat(420_000)).unwrap();

        let mode = preview_text(&file_path, 80);

        if let QuickViewMode::Text { lines, .. } = mode {
            assert!(lines.len() <= TEXT_PREVIEW_MAX_LINES + 1);
            assert_eq!(
                lines.last().map(String::as_str),
                Some("… preview truncated")
            );
        } else {
            panic!("Expected QuickView Text mode");
        }
    }

    #[test]
    fn test_preview_text_keeps_capped_single_line_content() {
        let base = create_temp_dir("quick_view_large_single_line");
        let file_path = base.join("large.json");
        fs::write(
            &file_path,
            "x".repeat((TEXT_PREVIEW_MAX_BYTES + 10) as usize),
        )
        .unwrap();

        let mode = preview_text(&file_path, 80);

        if let QuickViewMode::Text { lines, .. } = mode {
            assert!(lines.iter().any(|line| line.contains('x')));
            assert_eq!(
                lines.last().map(String::as_str),
                Some("… preview truncated")
            );
        } else {
            panic!("Expected QuickView Text mode");
        }
    }

    #[test]
    fn test_bat_lines_are_split_and_capped() {
        let output = (0..TEXT_PREVIEW_MAX_LINES + 10)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");

        let lines = bat_lines_from_output(output.as_bytes());

        assert_eq!(lines.len(), TEXT_PREVIEW_MAX_LINES);
        assert_eq!(lines[0], "line 0");
        assert_eq!(
            lines.last().map(String::as_str),
            Some(format!("line {}", TEXT_PREVIEW_MAX_LINES - 1).as_str())
        );
    }

    #[test]
    fn preview_image_downscales_large_images() {
        let base = create_temp_dir("quick_view_image_downscale");
        let file_path = base.join("huge.jpg");
        let img = image::RgbImage::from_fn(4000, 3000, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        });
        img.save(&file_path).unwrap();

        if let QuickViewMode::Image(bytes) = preview_image(&file_path, 60) {
            let original = fs::metadata(&file_path).unwrap().len() as usize;
            assert!(bytes.len() < original);
        } else {
            panic!("Expected QuickView Image mode");
        }
    }
}
