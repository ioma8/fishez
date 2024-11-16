use std::env;
use std::fs;
use std::path::PathBuf;
use std::path::MAIN_SEPARATOR;
use std::process::Command;

pub struct FilesView {
    pub files: Vec<String>,
    pub selected: usize,
    pub start: usize,
    pub pwd: String,
    pub mode: FilesViewMode,
    pub filter_string: String,
    pub content_lines: Vec<String>,
    pub content_start: usize,
}

#[derive(PartialEq)]
pub enum FilesViewMode {
    Normal,
    Filter,
    QuickView,
    RecursiveSearch,
    RipGrep,
}

pub static HEADER_ROWS: u16 = 2;
pub static FOOTER_ROWS: u16 = 3;

impl FilesView {
    pub fn new() -> Self {
        FilesView {
            files: vec![],
            selected: 0,
            start: 0,
            pwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
            filter_string: String::new(),
            mode: FilesViewMode::Normal,
            content_lines: vec![],
            content_start: 0,
        }
    }

    pub fn update(&mut self) {
        self.files = vec!["..".to_string()];
        self.files.extend(
            fs::read_dir(&self.pwd)
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    let file_name = entry.file_name().into_string().unwrap();
                    if entry.file_type().unwrap().is_dir() {
                        format!("{}/", file_name)
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

    pub fn scroll_content(&mut self, direction: isize) {
        let new_start = self.content_start as isize + direction;
        if new_start >= 0 && (new_start as usize) < self.content_lines.len() {
            self.content_start = new_start as usize;
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

    pub fn open_selected_file(&mut self) {
        let selected_file = self.files[self.selected].clone();
        if selected_file == ".." {
            self.go_up_one_level();
        } else if selected_file.ends_with('/') {
            self.change_directory(&selected_file);
        } else {
            self.open_file(&selected_file);
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
            selected_file.trim_end_matches('/')
        );
        self.update();
    }

    pub fn open_file(&self, selected_file: &str) {
        let file_path = format!("{}{}{}", self.pwd, MAIN_SEPARATOR, selected_file);
        if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", "start", "", &file_path])
                .spawn()
                .unwrap();
        } else if cfg!(target_os = "macos") {
            Command::new("open").arg(&file_path).spawn().unwrap();
        } else {
            Command::new("xdg-open").arg(&file_path).spawn().unwrap();
        }
    }

    pub fn open_in_editor(&self) {
        let selected_file = &self.files[self.selected];
        let file_path = format!("{}{}{}", self.pwd, MAIN_SEPARATOR, selected_file);

        if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", "start", "/B", "code", &file_path])
                .spawn()
                .unwrap();
        } else if cfg!(target_os = "macos") {
            Command::new("open")
                .arg("-a")
                .arg("Visual Studio Code")
                .arg(&file_path)
                .spawn()
                .unwrap();
        } else {
            Command::new("code").arg(&file_path).spawn().unwrap();
        }
    }

    pub fn toggle_quick_view(&mut self) {
        if self.mode == FilesViewMode::QuickView {
            self.mode = FilesViewMode::Normal;
        } else {
            let selected_file = &self.files[self.selected];
            let file_path = format!("{}{}{}", self.pwd, MAIN_SEPARATOR, selected_file);

            if !selected_file.ends_with('/') && fs::metadata(&file_path).is_ok() {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    self.content_lines = content.lines().map(String::from).collect();
                    self.content_start = 0;
                    self.mode = FilesViewMode::QuickView;
                } else {
                    println!("Could not read file: {}", selected_file);
                }
            }
        }
    }

    pub fn go_up_one_level(&mut self) {
        self.mode = FilesViewMode::Normal;
        self.filter_string.clear();
        let parent_dir = PathBuf::from(&self.pwd).parent().unwrap().to_path_buf();
        self.pwd = parent_dir.to_str().unwrap().to_string();
        self.update();
    }

    pub fn perform_recursive_search(&mut self) {
        let mut results = Vec::new();
        recursive_search(&self.pwd, &self.filter_string, &mut results);
        self.files = results;
    }

    pub fn perform_ripgrep_search(&mut self) {
        let mut results = Vec::new();
        ripgrep_search(&self.pwd, &self.filter_string, &mut results);
        self.files = results;
    }
}

fn recursive_search(dir: &str, query: &str, results: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                recursive_search(path.to_str().unwrap(), query, results);
            } else if let Ok(content) = fs::read_to_string(&path) {
                if content.contains(query) {
                    results.push(path.to_str().unwrap().to_string());
                }
            }
        }
    }
}

fn ripgrep_search(dir: &str, query: &str, results: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                ripgrep_search(path.to_str().unwrap(), query, results);
            } else if let Ok(content) = fs::read_to_string(&path) {
                if content.contains(query) {
                    results.push(path.to_str().unwrap().to_string());
                }
            }
        }
    }
}
