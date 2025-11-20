use std::collections::HashSet;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::path::MAIN_SEPARATOR;
use std::time::Duration;
use std::time::Instant;

use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;
use crossterm::style::Stylize;
use image::load_from_memory;
use little_exif::exif_tag::ExifTag;
use little_exif::metadata::Metadata;

use image::DynamicImage;
use image::ImageBuffer;
use textwrap;

use crate::logger::log;

#[derive(Debug)]
pub struct FilesView {
    pub files: Vec<String>,
    pub selected: usize,
    pub start: usize,
    pub pwd: String,
    pub mode: FilesViewMode,
    pub filter_string: String,
    pub notification: Option<String>,
    pub notification_created: Instant,
    multi_selected: HashSet<usize>,
}

#[derive(PartialEq, Debug)]
pub enum FilesViewMode {
    Normal,
    Filter,
    QuickView(QuickViewMode),
}

#[derive(PartialEq, Debug)]
pub enum QuickViewMode {
    Text {
        lines: Vec<String>,
        start: usize,
        length: usize,
    },
    Image(ImageBuffer<image::Rgb<u8>, Vec<u8>>, Vec<u8>),
    Directory {
        lines: Vec<String>,
    },
    NotSupported,
}

enum FileType {
    Text,
    Image,
    Other,
}

pub static HEADER_ROWS: u16 = 2;
pub static FOOTER_ROWS: u16 = 3;
pub static NOTIFICATION_TIMEOUT: usize = 3000;
const MIN_WRAP_WIDTH: u16 = 20;
const DIRECTORY_PREVIEW_LIMIT: usize = 20;

impl Default for FilesView {
    fn default() -> Self {
        Self::new()
    }
}

impl FilesView {
    pub fn new() -> Self {
        FilesView {
            files: vec![],
            selected: 0,
            start: 0,
            pwd: env::current_dir()
                .unwrap_or("/".into())
                .to_str()
                .unwrap_or("/")
                .to_string(),
            filter_string: String::new(),
            mode: FilesViewMode::Normal,
            notification: None,
            notification_created: Instant::now(),
            multi_selected: HashSet::new(),
        }
    }

    pub fn update_and_focus(&mut self, focus_on: &str) {
        self.update();
        self.selected = self.files.iter().position(|f| f == focus_on).unwrap_or(0);
        if self.selected >= self.start + 10 {
            self.start = self.selected - 10;
        }
    }

    pub fn update(&mut self) {
        self.multi_selected.clear();
        self.files = vec![];
        if self.mode == FilesViewMode::Normal {
            self.files.push("..".to_string());
        }

        self.files.extend(
            fs::read_dir(&self.pwd)
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    let file_name = entry.file_name().into_string().unwrap();
                    if entry.file_type().unwrap().is_dir() {
                        format!("{}{}", file_name, MAIN_SEPARATOR)
                    } else {
                        file_name
                    }
                })
                .filter(|name| {
                    self.filter_string.is_empty()
                        || name
                            .to_lowercase()
                            .contains(&self.filter_string.to_lowercase())
                })
                .collect::<Vec<String>>(),
        );
        self.selected = 0;
        self.start = 0;
    }

    pub fn set_notification(&mut self, notification: String) {
        self.notification = Some(notification);
        self.notification_created = Instant::now();
    }

    pub fn clear_notification_force(&mut self) {
        self.notification = None;
    }

    pub fn clear_notification(&mut self) {
        if self.notification.is_none() {
            return;
        }

        if self.notification_created.elapsed() > Duration::from_millis(NOTIFICATION_TIMEOUT as u64)
        {
            self.notification = None;
        }
    }

    pub fn reset_filter_mode(&mut self) {
        self.mode = FilesViewMode::Normal;
        self.filter_string.clear();
        self.update();
    }

    pub fn update_filter_string<F>(&mut self, update_fn: F)
    where
        F: FnOnce(&mut String),
    {
        update_fn(&mut self.filter_string);
        self.update();
    }

    pub fn scroll_content(&mut self, direction: isize, rows: u16) {
        if let FilesViewMode::QuickView(QuickViewMode::Text {
            lines,
            start,
            length,
        }) = &self.mode
        {
            let visible_rows = rows.saturating_sub(HEADER_ROWS + FOOTER_ROWS) as usize;
            let max_start = length.saturating_sub(visible_rows.max(1));
            let new_start = (*start as isize + direction).clamp(0, max_start as isize) as usize;

            self.mode = FilesViewMode::QuickView(QuickViewMode::Text {
                lines: lines.clone(),
                start: new_start,
                length: *length,
            });
        }
    }

    pub fn navigate(&mut self, direction: isize, rows_available: u16) {
        let new_selected = self.selected as isize + direction;
        if new_selected >= 0 && (new_selected as usize) < self.files.len() {
            self.selected = new_selected as usize;
            if self.selected < self.start {
                self.start = self.selected;
            } else if self.selected >= self.start + rows_available as usize {
                self.start = self.selected - rows_available as usize + 1;
            }
        }
    }

    pub fn navigate_home(&mut self) {
        self.selected = 0;
        self.start = 0;
    }

    pub fn navigate_end(&mut self, rows_available: u16) {
        self.selected = self.files.len() - 1;
        self.start = if self.files.len() > rows_available as usize {
            self.files.len() - rows_available as usize
        } else {
            0
        };
    }

    pub fn get_selected_file_abs(&self) -> Option<String> {
        if self.selected < self.files.len() {
            let selected_file = &self.files[self.selected];
            if selected_file == ".." {
                None
            } else {
                Some(format!("{}{}{}", self.pwd, MAIN_SEPARATOR, selected_file))
            }
        } else {
            None
        }
    }

    pub fn replace_files(&mut self, files: Vec<String>) {
        self.files = files;
        self.selected = 0;
        self.start = 0;
        self.mode = FilesViewMode::Normal;
        self.filter_string.clear();
    }

    pub fn toggle_multi_selection(&mut self, index: usize) {
        if let Some(name) = self.files.get(index) {
            if name == ".." {
                return;
            }
        }

        if self.multi_selected.contains(&index) {
            self.multi_selected.remove(&index);
        } else {
            self.multi_selected.insert(index);
        }
    }

    pub fn clear_multi_selection(&mut self) {
        self.multi_selected.clear();
    }

    pub fn is_multi_selected(&self, index: usize) -> bool {
        self.multi_selected.contains(&index)
    }

    pub fn multi_selected_count(&self) -> usize {
        self.multi_selected.len()
    }

    pub fn multi_selected_paths(&self) -> Vec<String> {
        let mut indexes: Vec<usize> = self.multi_selected.iter().copied().collect();
        indexes.sort_unstable();

        let len = self.files.len();
        indexes
            .into_iter()
            .filter(|idx| *idx < len)
            .filter_map(|idx| self.files.get(idx))
            .filter(|name| *name != "..")
            .map(|name| format!("{}{}{}", self.pwd, MAIN_SEPARATOR, name))
            .collect()
    }

    pub fn open_selected_file(&mut self) {
        let selected_file = self.files[self.selected].clone();
        if selected_file == ".." {
            self.go_up_one_level();
        } else if selected_file.ends_with(MAIN_SEPARATOR) {
            self.change_directory(&selected_file);
        }
    }

    pub fn change_directory(&mut self, selected_file: &str) {
        self.mode = FilesViewMode::Normal;
        self.filter_string.clear();
        let separator_string = MAIN_SEPARATOR.to_string();
        let separator = if self.pwd.ends_with(MAIN_SEPARATOR) {
            ""
        } else {
            separator_string.as_str()
        };
        self.pwd = format!(
            "{}{}{}",
            self.pwd,
            separator,
            selected_file.trim_end_matches(MAIN_SEPARATOR)
        );
        self.update();
    }

    pub fn copy_selected_to_clipboard(&mut self, absolute_path: bool) {
        let selected_file = &self.files[self.selected];
        let file_path = if absolute_path {
            format!("{}{}{}", self.pwd, MAIN_SEPARATOR, selected_file)
        } else {
            selected_file.clone()
        };

        let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
        ctx.set_contents(file_path.clone()).unwrap();
        if absolute_path {
            self.set_notification(format!("Copied absolute path to clipboard: {}", file_path));
        } else {
            self.set_notification(format!("Copied name to clipboard: {}", selected_file));
        }
    }

    pub fn open_quick_view(&mut self, wrap_width: u16) {
        if self.files.is_empty() {
            return;
        }
        let selected_file = self.files[self.selected].clone();
        let file_path = format!("{}{}{}", self.pwd, MAIN_SEPARATOR, selected_file);

        self.show_file_quick_view(file_path, &selected_file, wrap_width);
    }

    fn get_type_from_path(&self, file_path: &String) -> FileType {
        if let Some(mime_type) = tree_magic_mini::from_filepath(std::path::Path::new(&file_path)) {
            match mime_type.split('/').next() {
                Some("text") => FileType::Text,
                Some("image") => FileType::Image,
                _ => {
                    match std::path::Path::new(file_path)
                        .extension()
                        .and_then(std::ffi::OsStr::to_str)
                    {
                        Some("txt") | Some("md") | Some("rs") | Some("toml") => FileType::Text,
                        Some("png") | Some("jpg") | Some("jpeg") | Some("gif") => FileType::Image,
                        _ => FileType::Other,
                    }
                }
            }
        } else {
            FileType::Other
        }
    }

    fn show_file_quick_view(&mut self, file_path: String, selected_file: &String, wrap_width: u16) {
        let meta = fs::metadata(&file_path).is_ok();

        if !meta {
            self.mode = FilesViewMode::QuickView(QuickViewMode::NotSupported);
            return;
        }

        if file_path.ends_with(MAIN_SEPARATOR) {
            self.show_file_quick_view_directory(file_path.clone());
            return;
        }

        let ftype = self.get_type_from_path(&file_path);
        match ftype {
            FileType::Text => self.show_file_quick_view_text(file_path.clone(), selected_file, wrap_width),
            FileType::Image => self.show_file_quick_view_image(file_path.clone()),
            _ => self.show_file_quick_view_not_supported(file_path.clone()),
        }
    }

    fn show_file_quick_view_text(&mut self, file_path: String, selected_file: &String, wrap_width: u16) {
        if let Ok(content) = fs::read_to_string(&file_path) {
            // TODO: předávat asi přímo Reader namísto celého filu ve stringu
            let width = wrap_width.saturating_sub(4).max(MIN_WRAP_WIDTH) as usize;
            let lines: Vec<String> = textwrap::wrap(&content, width)
                .into_iter()
                .map(|line| line.to_string())
                .collect();
            // TODO: ve filu implementovat End / Home pro přesun na začátek a konec souboru kk
            let length = lines.len();
            self.mode = FilesViewMode::QuickView(QuickViewMode::Text {
                lines: self.syntax_highlight_text(lines),
                start: 0,
                length,
            });
        } else {
            log(&format!("Could not read file: {}", selected_file));
        }
    }

    fn syntax_highlight_text(&self, lines: Vec<String>) -> Vec<String> {
        let comment_markers = ["//", "#", "--"];
        let keyword_markers = vec![
            "fn", "let", "if", "else", "for", "while", "match", "struct", "enum", "impl",
            "function", "trait", "mod", "pub", "private", "self", "super", "const", "var",
            "static", "type", "async", "await", "return", "break", "continue", "match", "loop",
            "in", "as", "where", "crate", "extern", "dyn", "ref", "mut",
        ];
        let keywords_fullline = ["derive", "use", "import"];

        let mut comment_started = false;
        let mut fullline_keyword_started = false;

        lines
            .iter()
            .map(|line| {
                comment_started = false;
                fullline_keyword_started = false;
                line.split(" ")
                    .map(|word| {
                        if comment_started
                            || comment_markers
                                .iter()
                                .any(|marker| word.starts_with(marker))
                        {
                            comment_started = true;
                            format!("{} ", word.with(crossterm::style::Color::DarkGreen))
                        } else if keyword_markers.contains(&word) {
                            format!("{} ", word.with(crossterm::style::Color::Yellow))
                        } else if fullline_keyword_started || keywords_fullline.contains(&word) {
                            fullline_keyword_started = true;
                            format!("{} ", word.with(crossterm::style::Color::Cyan))
                        } else {
                            word.to_string()
                        }
                    })
                    .collect::<Vec<String>>()
                    .join(" ")
            })
            .collect::<Vec<String>>()
    }

    fn show_file_quick_view_directory(&mut self, file_path: String) {
        let mut files = vec![];
        let mut dirs = vec![];
        let mut total_size: u64 = 0;
        let mut total_entries = 0usize;

        let entries = fs::read_dir(&file_path);
        if let Ok(entries) = entries {
            for entry in entries.flatten() {
                total_entries += 1;
                let file_name = entry.file_name().into_string().unwrap_or_default();
                let meta = entry.metadata().ok();
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    dirs.push(file_name);
                } else {
                    if let Some(len) = meta.as_ref().map(|m| m.len()) {
                        total_size += len;
                    }
                    files.push(file_name);
                }
            }
        }

        dirs.sort();
        files.sort();

        let mut lines = vec![];
        lines.push(format!("Directory: {}", file_path));
        lines.push(format!(
            "Entries: {} (dirs: {}, files: {})",
            total_entries,
            dirs.len(),
            files.len()
        ));
        lines.push(format!("Size (files only): {}", Self::human_readable_size(total_size)));
        lines.push(String::new());

        if !dirs.is_empty() {
            lines.push("Directories:".into());
            for dir in dirs.iter().take(DIRECTORY_PREVIEW_LIMIT) {
                lines.push(format!("{}/", dir));
            }
            if dirs.len() > DIRECTORY_PREVIEW_LIMIT {
                lines.push(format!(
                    "... and {} more",
                    dirs.len() - DIRECTORY_PREVIEW_LIMIT
                ));
            }
            lines.push(String::new());
        }

        if !files.is_empty() {
            lines.push("Files:".into());
            for file in files.iter().take(DIRECTORY_PREVIEW_LIMIT) {
                lines.push(file.to_string());
            }
            if files.len() > DIRECTORY_PREVIEW_LIMIT {
                lines.push(format!(
                    "... and {} more",
                    files.len() - DIRECTORY_PREVIEW_LIMIT
                ));
            }
        }

        self.mode = FilesViewMode::QuickView(QuickViewMode::Directory { lines });
    }

    fn show_file_quick_view_not_supported(&mut self, _: String) {
        self.mode = FilesViewMode::QuickView(QuickViewMode::NotSupported);
    }

    fn extract_embedded_thumbnail(&self, path: &str) -> Option<DynamicImage> {
        let image_path = std::path::Path::new(path);
        let metadata = Metadata::new_from_path(image_path).ok()?;

        let thumb_offset = metadata
            .get_tag(&ExifTag::ThumbnailOffset(vec![], vec![]))
            .next()?;

        let thumb_data = if let ExifTag::ThumbnailOffset(_, data) = thumb_offset {
            data
        } else {
            return None;
        };

        let img = load_from_memory(thumb_data).ok()?;
        Some(img)
    }

    fn show_file_quick_view_image(&mut self, file_path: String) {
        let now = Instant::now();
        let thumb = self.extract_embedded_thumbnail(&file_path);
        let image_pixels = if let Some(thumb) = thumb {
            Some(thumb.to_rgb8())
        } else {
            match image::open(&file_path) {
                Ok(img) => Some(img.to_rgb8()),
                _ => None,
            }
        };
        let mut f = File::open(&file_path).expect("Could not open image file");
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        log(&format!("Image loading took: {:?}", now.elapsed()));

        if let Some(image_pixels) = image_pixels {
            self.mode = FilesViewMode::QuickView(QuickViewMode::Image(image_pixels, buf));
        }
    }

    fn human_readable_size(bytes: u64) -> String {
        const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit = 0;
        while size >= 1024.0 && unit < UNITS.len() - 1 {
            size /= 1024.0;
            unit += 1;
        }
        if unit == 0 {
            format!("{} {}", bytes, UNITS[unit])
        } else {
            format!("{:.2} {}", size, UNITS[unit])
        }
    }

    pub fn go_up_one_level(&mut self) {
        self.mode = FilesViewMode::Normal;
        self.filter_string.clear();
        let old_dir = self.pwd.clone();
        if let Some(parent_dir) = PathBuf::from(&self.pwd).parent() {
            let parent_dir = parent_dir.to_str().unwrap();
            let old_dir = old_dir
                .trim_start_matches(parent_dir)
                .trim_start_matches(MAIN_SEPARATOR)
                .trim_end_matches(MAIN_SEPARATOR);
            self.pwd = parent_dir.to_string();
            let old_dir = format!("{}{}", old_dir, MAIN_SEPARATOR);
            self.update_and_focus(&old_dir);
        }
    }
}
