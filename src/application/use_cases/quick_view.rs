//! Quick view use case - file preview.

use crate::application::{PanelMode, PanelState, QuickViewMode};
use image::{DynamicImage, load_from_memory};
use little_exif::exif_tag::ExifTag;
use little_exif::metadata::Metadata;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MIN_WRAP_WIDTH: u16 = 20;
const DIR_PREVIEW_LIMIT: usize = 20;

pub fn open(panel: &mut PanelState, wrap_width: u16) {
    if panel.entries.is_empty() || panel.cursor >= panel.entries.len() {
        return;
    }
    let path = panel.entries[panel.cursor].path.clone();
    show_file(panel, path, wrap_width);
}

pub fn scroll(panel: &mut PanelState, direction: isize, rows: u16, header: u16, footer: u16) {
    if let PanelMode::QuickView(QuickViewMode::Text {
        lines,
        start,
        length,
    }) = &panel.mode
    {
        let visible = rows.saturating_sub(header + footer) as usize;
        let max = length.saturating_sub(visible.max(1));
        let new = (*start as isize + direction).clamp(0, max as isize) as usize;
        panel.mode = PanelMode::QuickView(QuickViewMode::Text {
            lines: lines.clone(),
            start: new,
            length: *length,
        });
    }
}

fn show_file(panel: &mut PanelState, path: PathBuf, wrap_width: u16) {
    if fs::metadata(&path).is_err() {
        panel.mode = PanelMode::QuickView(QuickViewMode::NotSupported);
        return;
    }
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
        let length = lines.len();
        panel.mode = PanelMode::QuickView(QuickViewMode::Text {
            lines: highlight(lines),
            start: 0,
            length,
        });
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
    let thumb = extract_thumbnail(path);
    let pixels = thumb
        .map(|t| t.to_rgb8())
        .or_else(|| image::open(path).ok().map(|i| i.to_rgb8()));
    let Ok(mut f) = File::open(path) else { return };
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return;
    }
    crate::logger::log(&format!("Image loading took: {:?}", now.elapsed()));
    if let Some(px) = pixels {
        panel.mode = PanelMode::QuickView(QuickViewMode::Image(px.into_raw(), buf));
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
