use crossterm::style::{Color, Stylize};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute, queue, terminal,
};
use std::env;
use std::io::{stdout, Write};
use std::panic;
use std::path::PathBuf;
use std::{fs, path::MAIN_SEPARATOR, process::Command};

struct FilesView {
    files: Vec<String>,
    selected: usize,
    start: usize,
    pwd: String,
    mode: FilewViewMode,
    filter_string: String,
    content_lines: Vec<String>,
    content_start: usize,
}

#[derive(PartialEq)]
enum FilewViewMode {
    Normal,
    Filter,
    QuickView,
    RecursiveSearch,
    RipGrep,
}

struct TerminalUI {
    columns: u16,
    rows: u16,
}

static HEADER_ROWS: u16 = 2;
static FOOTER_ROWS: u16 = 3;

impl FilesView {
    fn new() -> Self {
        FilesView {
            files: vec![],
            selected: 0,
            start: 0,
            pwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
            filter_string: String::new(),
            mode: FilewViewMode::Normal,
            content_lines: vec![],
            content_start: 0,
        }
    }

    fn update(&mut self) {
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

    fn reset_filter_mode(&mut self) {
        self.mode = FilewViewMode::Normal;
        self.filter_string.clear();
        self.update();
    }

    fn update_filter_string<F>(&mut self, update_fn: F)
    where
        F: FnOnce(&mut String),
    {
        update_fn(&mut self.filter_string);
        self.update();
    }

    fn scroll_content(&mut self, direction: isize) {
        let new_start = self.content_start as isize + direction;
        if new_start >= 0 && (new_start as usize) < self.content_lines.len() {
            self.content_start = new_start as usize;
        }
    }

    fn navigate(&mut self, direction: isize, rows_available: u16) {
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

    fn navigate_home(&mut self) {
        self.selected = 0;
        self.start = 0;
    }

    fn navigate_end(&mut self, rows_available: u16) {
        self.selected = self.files.len() - 1;
        self.start = if self.files.len() > rows_available as usize {
            self.files.len() - rows_available as usize
        } else {
            0
        };
    }

    fn open_selected_file(&mut self) {
        let selected_file = self.files[self.selected].clone();
        if selected_file == ".." {
            self.go_up_one_level();
        } else if selected_file.ends_with('/') {
            self.change_directory(&selected_file);
        } else {
            self.open_file(&selected_file);
        }
    }

    fn change_directory(&mut self, selected_file: &str) {
        self.mode = FilewViewMode::Normal;
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

    fn open_file(&self, selected_file: &str) {
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

    fn open_in_editor(&self) {
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

    fn toggle_quick_view(&mut self) {
        if self.mode == FilewViewMode::QuickView {
            self.mode = FilewViewMode::Normal;
        } else {
            let selected_file = &self.files[self.selected];
            let file_path = format!("{}{}{}", self.pwd, MAIN_SEPARATOR, selected_file);

            if !selected_file.ends_with('/') && fs::metadata(&file_path).is_ok() {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    self.content_lines = content.lines().map(String::from).collect();
                    self.content_start = 0;
                    self.mode = FilewViewMode::QuickView;
                } else {
                    println!("Could not read file: {}", selected_file);
                }
            }
        }
    }

    fn go_up_one_level(&mut self) {
        self.mode = FilewViewMode::Normal;
        self.filter_string.clear();
        let parent_dir = PathBuf::from(&self.pwd).parent().unwrap().to_path_buf();
        self.pwd = parent_dir.to_str().unwrap().to_string();
        self.update();
    }

    fn perform_recursive_search(&mut self) {
        let mut results = Vec::new();
        recursive_search(&self.pwd, &self.filter_string, &mut results);
        self.files = results;
    }

    fn perform_ripgrep_search(&mut self) {
        let mut results = Vec::new();
        ripgrep_search(&self.pwd, &self.filter_string, &mut results);
        self.files = results;
    }
}

impl TerminalUI {
    fn new() -> Self {
        let (columns, rows) = terminal::size().unwrap();
        TerminalUI { columns, rows }
    }

    fn draw_ui(&self, stdout: &mut std::io::Stdout, files_view: &FilesView) {
        let _ = execute!(
            stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::DisableBlinking,
            cursor::Hide
        );
        let _ = queue!(stdout, cursor::MoveTo(0, 0));

        self.draw_header(stdout, files_view);

        if files_view.mode == FilewViewMode::QuickView {
            self.draw_file_content(files_view);
        } else {
            self.draw_files_list(files_view);
        }

        self.draw_footer(stdout, files_view);
        let _ = stdout.flush();
    }

    fn draw_header(&self, stdout: &mut std::io::Stdout, files_view: &FilesView) {
        if files_view.mode == FilewViewMode::QuickView {
            println!(
                "{}",
                format!("Viewing: {}", &files_view.files[files_view.selected]).with(Color::Cyan)
            );
        } else {
            println!("{}", format!("PWD: {}", files_view.pwd).with(Color::Cyan));
        }
        let title = "FISHEZ";
        let _ = queue!(stdout, cursor::MoveTo(self.columns - title.len() as u16, 0));
        println!("{}", title.with(Color::Cyan));
        self.draw_full_line();
    }

    fn draw_files_list(&self, files_view: &FilesView) {
        let rows_available = self.rows - HEADER_ROWS - FOOTER_ROWS;
        let files_to_display = files_view.files[files_view.start..]
            .iter()
            .take(rows_available as usize);

        for (i, file) in files_to_display.enumerate() {
            let name = if file.ends_with('/') {
                file.as_str().yellow()
            } else {
                file.as_str().dark_yellow()
            };

            let name_final = if i == files_view.selected - files_view.start {
                name.negative()
            } else {
                name
            };

            println!("{}", name_final);
        }
    }

    fn draw_file_content(&self, files_view: &FilesView) {
        let rows_available = self.rows - HEADER_ROWS - FOOTER_ROWS;

        let content_to_display = files_view.content_lines[files_view.content_start..]
            .iter()
            .take(rows_available as usize);

        for line in content_to_display {
            println!("{}", line);
        }
    }

    fn draw_footer(&self, stdout: &mut std::io::Stdout, files_view: &FilesView) {
        queue!(stdout, cursor::MoveTo(0, self.rows - FOOTER_ROWS)).unwrap();
        self.draw_full_line();

        if files_view.mode == FilewViewMode::Filter
            || files_view.mode == FilewViewMode::RecursiveSearch
            || files_view.mode == FilewViewMode::RipGrep
        {
            let filter_name = match files_view.mode {
                FilewViewMode::Filter => "Filter",
                FilewViewMode::RecursiveSearch => "Search",
                FilewViewMode::RipGrep => "RipGrep",
                _ => "",
            };
            println!(
                "{}",
                format!("{}: {}", filter_name, files_view.filter_string).with(Color::Green)
            );
        } else {
            let total_dirs = files_view
                .files
                .iter()
                .filter(|name| name.ends_with('/'))
                .count();
            let total_files = files_view.files.len() - total_dirs - 1;
            println!(
                "{}",
                format!("{} dirs, {} files", total_dirs, total_files).with(Color::Green)
            );
        }
        let actions = [
            "[s]earch",
            "[f]ind",
            "[r]ipgrep",
            "[f3]view",
            "[f4]edit",
            "[q]uit",
        ];
        let actions_str = actions.join(" ");
        let padding = (self.columns as usize - actions_str.len()) / (actions.len() - 1);
        let padded_actions = actions.join(&" ".repeat(padding));
        print!("{}", padded_actions.with(Color::Green));
    }

    fn draw_full_line(&self) {
        let text = (0..self.columns)
            .map(|_| "─")
            .collect::<String>()
            .with(Color::Blue);
        println!("{}", text);
    }
}

fn main() {
    setup_terminal();
    let mut stdout = stdout();
    let ui = TerminalUI::new();
    let mut files_view = FilesView::new();
    files_view.update();

    loop {
        ui.draw_ui(&mut stdout, &files_view);

        if let Event::Key(KeyEvent { code, kind, .. }) = event::read().unwrap() {
            if kind != event::KeyEventKind::Press {
                continue;
            }

            handle_key_event(&mut files_view, code, ui.rows);
        }
    }
}

fn setup_terminal() {
    ctrlc::set_handler(|| {
        reset_terminal();
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    panic::set_hook(Box::new(|_| {
        reset_terminal();
    }));
}

fn reset_terminal() {
    let _ = execute!(
        stdout(),
        cursor::MoveTo(0, 0),
        cursor::Show,
        cursor::EnableBlinking,
        terminal::Clear(terminal::ClearType::All)
    );
}

fn handle_key_event(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match files_view.mode {
        FilewViewMode::Normal => handle_normal_mode(files_view, code, rows),
        FilewViewMode::Filter => handle_filter_mode(files_view, code, rows),
        FilewViewMode::QuickView => handle_quick_view_mode(files_view, code),
        FilewViewMode::RecursiveSearch => handle_recursive_search_mode(files_view, code, rows),
        FilewViewMode::RipGrep => handle_ripgrep_mode(files_view, code, rows),
    }
}

fn handle_normal_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Char('s') => files_view.mode = FilewViewMode::Filter,
        KeyCode::Char('f') => files_view.mode = FilewViewMode::RecursiveSearch,
        KeyCode::Char('r') => files_view.mode = FilewViewMode::RipGrep,
        KeyCode::F(3) => files_view.toggle_quick_view(),
        KeyCode::F(4) => files_view.open_in_editor(),
        KeyCode::Up => files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Down => files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Home => files_view.navigate_home(),
        KeyCode::End => files_view.navigate_end(rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Backspace => files_view.go_up_one_level(),
        KeyCode::Enter => files_view.open_selected_file(),
        KeyCode::Char('q') => {
            reset_terminal();
            std::process::exit(0);
        }
        _ => {}
    }
}

fn handle_filter_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Esc => files_view.reset_filter_mode(),
        KeyCode::Backspace => files_view.update_filter_string(|s| {
            s.pop();
        }),
        KeyCode::Char(c) => files_view.update_filter_string(|s| {
            s.push(c);
        }),
        KeyCode::Up => files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Down => files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Home => files_view.navigate_home(),
        KeyCode::End => files_view.navigate_end(rows - HEADER_ROWS - FOOTER_ROWS),
        _ => {}
    }
}

fn handle_quick_view_mode(files_view: &mut FilesView, code: KeyCode) {
    match code {
        KeyCode::Up => files_view.scroll_content(-1),
        KeyCode::Down => files_view.scroll_content(1),
        KeyCode::Esc | KeyCode::F(3) => files_view.mode = FilewViewMode::Normal,
        _ => {}
    }
}

fn handle_recursive_search_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Esc => files_view.reset_filter_mode(),
        KeyCode::Char(c) => {
            files_view.filter_string.push(c);
            files_view.perform_recursive_search();
        }
        KeyCode::Backspace => {
            files_view.filter_string.pop();
            files_view.perform_recursive_search();
        }
        KeyCode::Up => files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Down => files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Home => files_view.navigate_home(),
        KeyCode::End => files_view.navigate_end(rows - HEADER_ROWS - FOOTER_ROWS),
        _ => {}
    }
}

fn handle_ripgrep_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Esc => files_view.reset_filter_mode(),
        KeyCode::Char(c) => {
            files_view.filter_string.push(c);
            files_view.perform_ripgrep_search();
        }
        KeyCode::Backspace => {
            files_view.filter_string.pop();
            files_view.perform_ripgrep_search();
        }
        KeyCode::Up => files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Down => files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Home => files_view.navigate_home(),
        KeyCode::End => files_view.navigate_end(rows - HEADER_ROWS - FOOTER_ROWS),
        _ => {}
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
