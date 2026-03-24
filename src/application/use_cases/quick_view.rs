//! Quick view use case - file preview.

use crate::application::{PanelMode, PanelState, QuickViewMode};
use image::{DynamicImage, load_from_memory};
use little_exif::exif_tag::ExifTag;
use little_exif::metadata::Metadata;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MIN_WRAP_WIDTH: u16 = 20;
const DIR_PREVIEW_LIMIT: usize = 20;

pub fn open(panel: &mut PanelState, wrap_width: u16) {
    if let Some(path) = panel.get_selected_path() {
        show_file(panel, path, wrap_width);
    }
}

pub fn scroll(panel: &mut PanelState, direction: isize, rows: u16, header: u16, footer: u16) {
    if let PanelMode::QuickView(QuickViewMode::Text { lines, start }) = &mut panel.mode {
        let visible = rows.saturating_sub(header + footer) as usize;
        let max = lines.len().saturating_sub(visible.max(1));
        *start = (*start as isize + direction).clamp(0, max as isize) as usize;
    }
}

fn show_file(panel: &mut PanelState, path: PathBuf, wrap_width: u16) {
    if path.is_dir() {
        show_directory(panel, &path);
        return;
    }
    match get_type(&path) {
        FileType::Text => show_text(panel, &path, wrap_width),
        FileType::Image => show_image(panel, &path),
        FileType::Other => panel.mode = PanelMode::QuickView(QuickViewMode::NotSupported),
    }
}

enum FileType {
    Text,
    Image,
    Other,
}

fn get_type(path: &Path) -> FileType {
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
        FileType::Other
    }
}

fn show_text(panel: &mut PanelState, path: &Path, wrap_width: u16) {
    if let Ok(content) = fs::read_to_string(path) {
        let width = wrap_width.saturating_sub(4).max(MIN_WRAP_WIDTH) as usize;
        let lines: Vec<String> = textwrap::wrap(&content, width)
            .into_iter()
            .map(|l| l.to_string())
            .collect();
        panel.mode = PanelMode::QuickView(QuickViewMode::Text {
            lines: highlight(lines),
            start: 0,
        });
    } else {
        panel.mode = PanelMode::QuickView(QuickViewMode::NotSupported);
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

fn show_directory(panel: &mut PanelState, path: &Path) {
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
    panel.mode = PanelMode::QuickView(QuickViewMode::Directory { lines });
}

fn show_image(panel: &mut PanelState, path: &Path) {
    let now = Instant::now();
    let Ok(buf) = fs::read(path) else {
        panel.mode = PanelMode::QuickView(QuickViewMode::NotSupported);
        return;
    };
    let thumb = extract_thumbnail(path);
    let pixels = thumb
        .map(|t| t.to_rgb8())
        .or_else(|| load_from_memory(&buf).ok().map(|i| i.to_rgb8()));
    crate::logger::log(&format!("Image loading took: {:?}", now.elapsed()));
    if pixels.is_some() {
        panel.mode = PanelMode::QuickView(QuickViewMode::Image(buf));
    } else {
        panel.mode = PanelMode::QuickView(QuickViewMode::NotSupported);
    }
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
    fn test_show_text_sets_quick_view_mode() {
        let base = create_temp_dir("quick_view_text");
        let file_path = base.join("note.txt");
        fs::write(&file_path, "hello world").unwrap();

        let mut panel = PanelState::new();
        show_text(&mut panel, &file_path, 10);
        assert!(matches!(
            panel.mode,
            PanelMode::QuickView(QuickViewMode::Text { .. })
        ));
    }

    #[test]
    fn test_show_text_uses_min_wrap_width() {
        let base = create_temp_dir("quick_view_wrap");
        let file_path = base.join("long.txt");
        fs::write(&file_path, "word ".repeat(100)).unwrap();

        let mut panel = PanelState::new();
        show_text(&mut panel, &file_path, 5);
        if let PanelMode::QuickView(QuickViewMode::Text { lines, .. }) = &panel.mode {
            assert!(!lines.is_empty());
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

        let mut panel = PanelState::new();
        show_directory(&mut panel, &base);
        if let PanelMode::QuickView(QuickViewMode::Directory { lines }) = &panel.mode {
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
}
