//! Quick view use case - file preview.

use crate::application::use_cases::raw_image;
use crate::application::{PanelMode, PanelState, QuickViewMode};
use image::{DynamicImage, codecs::jpeg::JpegEncoder, imageops, load_from_memory};
use little_exif::exif_tag::ExifTag;
use little_exif::metadata::Metadata;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MIN_WRAP_WIDTH: u16 = 20;
const DIR_PREVIEW_LIMIT: usize = 20;

pub fn open(panel: &mut PanelState, wrap_width: u16) {
    if let Some(path) = panel.get_selected_path() {
        panel.mode = PanelMode::QuickView(preview(path, wrap_width));
    }
}

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

    if let Some(mime) = tree_magic_mini::from_filepath(path) {
        match mime.split('/').next() {
            Some("text") => FileType::Text,
            Some("image") => FileType::Image,
            _ => match path.extension().and_then(std::ffi::OsStr::to_str) {
                Some("txt") | Some("md") | Some("rs") | Some("toml") => FileType::Text,
                Some("png") | Some("jpg") | Some("jpeg") | Some("gif") => FileType::Image,
                _ => FileType::Other,
            },
        }
    } else {
        match path.extension().and_then(std::ffi::OsStr::to_str) {
            Some("txt") | Some("md") | Some("rs") | Some("toml") => FileType::Text,
            Some("png") | Some("jpg") | Some("jpeg") | Some("gif") => FileType::Image,
            _ => FileType::Other,
        }
    }
}

fn preview_text(path: &Path, wrap_width: u16) -> QuickViewMode {
    if let Ok(content) = fs::read_to_string(path) {
        let width = wrap_width.saturating_sub(4).max(MIN_WRAP_WIDTH) as usize;
        let lines: Vec<String> = textwrap::wrap(&content, width)
            .into_iter()
            .map(|l| l.to_string())
            .collect();
        QuickViewMode::Text {
            lines: highlight(lines),
            start: 0,
        }
    } else {
        QuickViewMode::NotSupported
    }
}

fn highlight(lines: Vec<String>) -> Vec<String> {
    use crossterm::style::Stylize;
    let comments = ["//", "#", "--"];
    let keywords = [
        "fn", "let", "if", "else", "for", "while", "match", "struct", "enum", "impl", "function",
        "trait", "mod", "pub", "private", "self", "super", "const", "var", "static", "type",
        "async", "await", "return", "break", "continue", "loop", "in", "as", "where", "crate",
        "extern", "dyn", "ref", "mut",
    ];
    let fullline = ["derive", "use", "import"];
    lines
        .iter()
        .map(|line| {
            let mut in_comment = false;
            let mut in_fullline = false;
            line.split(' ')
                .map(|w| {
                    if in_comment || comments.iter().any(|c| w.starts_with(c)) {
                        in_comment = true;
                        format!("{} ", w.with(crossterm::style::Color::DarkGreen))
                    } else if keywords.contains(&w) {
                        format!("{} ", w.with(crossterm::style::Color::Yellow))
                    } else if in_fullline || fullline.contains(&w) {
                        in_fullline = true;
                        format!("{} ", w.with(crossterm::style::Color::Cyan))
                    } else {
                        w.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
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
        crate::logger::log("Raw image rendered via jpgfromrawlib");
        return QuickViewMode::Image(raw_bytes);
    }
    let now = Instant::now();
    let Ok(buf) = fs::read(path) else {
        return QuickViewMode::NotSupported;
    };
    let thumb = extract_thumbnail(path);
    let pixels = thumb
        .map(|t| t.to_rgb8())
        .or_else(|| load_from_memory(&buf).ok().map(|i| i.to_rgb8()));
    crate::logger::log(&format!("Image loading took: {:?}", now.elapsed()));
    if pixels.is_some() {
        let target = terminal_pixel_limit(wrap_width);
        let image_bytes = downscale_image_if_needed(&buf, target).unwrap_or(buf);
        QuickViewMode::Image(image_bytes)
    } else {
        QuickViewMode::NotSupported
    }
}

fn terminal_pixel_limit(wrap_width: u16) -> u32 {
    const PIXELS_PER_COLUMN: u32 = 8;
    const MAX_SIDE: u32 = 2048;
    let side = wrap_width as u32 * PIXELS_PER_COLUMN;
    side.min(MAX_SIDE)
}

fn downscale_image_if_needed(bytes: &[u8], max_side: u32) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    if img.width() <= max_side && img.height() <= max_side {
        return None;
    }
    let resized = imageops::thumbnail(&img, max_side, max_side);
    let mut output = Vec::new();
    let _ = JpegEncoder::new_with_quality(&mut output, 80)
        .encode_image(&resized)
        .ok()?;
    Some(output)
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

fn human_size(bytes: u64) -> String {
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
