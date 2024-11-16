use crossterm::style::{Color, PrintStyledContent, Stylize};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute, queue, terminal,
};
use ctrlc;
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

static HEADER_ROWS: u16 = 2;
static FOOTER_ROWS: u16 = 3;

fn main() {
    setup_terminal();
    let mut stdout = stdout();
    let (columns, rows) = terminal::size().unwrap();

    let mut files_view = FilesView::new();
    update_files_view(&mut files_view);

    loop {
        draw_ui(&mut stdout, &files_view, columns, rows);

        if let Event::Key(KeyEvent { code, kind, .. }) = event::read().unwrap() {
            if kind != event::KeyEventKind::Press {
                continue;
            }

            handle_key_event(&mut files_view, code, rows);
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
}

fn draw_ui(stdout: &mut std::io::Stdout, files_view: &FilesView, columns: u16, rows: u16) {
    let _ = execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::DisableBlinking,
        cursor::Hide
    );
    let _ = queue!(stdout, cursor::MoveTo(0, 0));

    draw_header(stdout, files_view, columns);

    if files_view.mode == FilewViewMode::QuickView {
        draw_file_content(files_view, rows);
    } else {
        draw_files_list(files_view, rows);
    }

    draw_footer(stdout, files_view, columns, rows);
    let _ = stdout.flush();
}

fn draw_header(stdout: &mut std::io::Stdout, files_view: &FilesView, columns: u16) {
    if files_view.mode == FilewViewMode::QuickView {
        println!(
            "{}",
            format!("Viewing: {}", &files_view.files[files_view.selected]).with(Color::Cyan)
        );
    } else {
        println!("{}", format!("PWD: {}", files_view.pwd).with(Color::Cyan));
    }
    let title = "FISHEZ";
    let _ = queue!(stdout, cursor::MoveTo(columns - title.len() as u16, 0));
    println!("{}", title.with(Color::Cyan));
    draw_full_line(columns);
}

fn draw_files_list(files_view: &FilesView, rows: u16) {
    let rows_available = rows - HEADER_ROWS - FOOTER_ROWS;
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

fn draw_file_content(files_view: &FilesView, rows: u16) {
    let rows_available = rows - HEADER_ROWS - FOOTER_ROWS;

    let content_to_display = files_view.content_lines[files_view.content_start..]
        .iter()
        .take(rows_available as usize);

    for line in content_to_display {
        println!("{}", line);
    }
}

fn draw_footer(stdout: &mut std::io::Stdout, files_view: &FilesView, columns: u16, rows: u16) {
    queue!(stdout, cursor::MoveTo(0, rows - FOOTER_ROWS)).unwrap();
    draw_full_line(columns);

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
    let actions = vec![
        "[s]earch",
        "[f]ind",
        "[r]ipgrep",
        "[f3]view",
        "[f4]edit",
        "[q]uit",
    ];
    let actions_str = actions.join(" ");
    let padding = (columns as usize - actions_str.len()) / (actions.len() - 1);
    let padded_actions = actions.join(&" ".repeat(padding));
    print!("{}", padded_actions.with(Color::Green));
}

fn handle_key_event(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match files_view.mode {
        FilewViewMode::Normal => handle_normal_mode(files_view, code, rows),
        FilewViewMode::Filter => handle_filter_mode(files_view, code, rows),
        FilewViewMode::QuickView => handle_quick_view_mode(files_view, code, rows),
        FilewViewMode::RecursiveSearch => handle_recursive_search_mode(files_view, code),
        FilewViewMode::RipGrep => handle_ripgrep_mode(files_view, code),
    }
}

fn handle_normal_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Char('s') => files_view.mode = FilewViewMode::Filter,
        KeyCode::Char('f') => files_view.mode = FilewViewMode::RecursiveSearch,
        KeyCode::Char('r') => files_view.mode = FilewViewMode::RipGrep,
        _ => handle_navigation_keys(files_view, code, rows - HEADER_ROWS - FOOTER_ROWS),
    }
}

fn handle_filter_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Esc => reset_filter_mode(files_view),
        KeyCode::Backspace => update_filter_string(files_view, |s| {
            s.pop();
        }),
        KeyCode::Char(c) => update_filter_string(files_view, |s| {
            s.push(c);
        }),
        _ => handle_navigation_keys(files_view, code, rows - HEADER_ROWS - FOOTER_ROWS),
    }
}

fn handle_quick_view_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Up => scroll_content(files_view, -1),
        KeyCode::Down => scroll_content(files_view, 1),
        KeyCode::Esc | KeyCode::F(3) => files_view.mode = FilewViewMode::Normal,
        _ => {}
    }
}

fn handle_recursive_search_mode(files_view: &mut FilesView, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            reset_filter_mode(files_view);
        }
        KeyCode::Char(c) => {
            files_view.filter_string.push(c);
            perform_recursive_search(files_view);
        }
        KeyCode::Backspace => {
            files_view.filter_string.pop();
            perform_recursive_search(files_view);
        }
        _ => todo!("handle navigation keys"),
    }
}

fn handle_ripgrep_mode(files_view: &mut FilesView, code: KeyCode) {
    match code {
        KeyCode::Esc => reset_filter_mode(files_view),
        KeyCode::Char(c) => {
            files_view.filter_string.push(c);
            perform_ripgrep_search(files_view);
        }
        KeyCode::Backspace => {
            files_view.filter_string.pop();
            perform_ripgrep_search(files_view);
        }
        _ => todo!("handle navigation keys"),
    }
}

fn handle_navigation_keys(files_view: &mut FilesView, code: KeyCode, rows_available: u16) {
    match code {
        KeyCode::F(3) => toggle_quick_view(files_view),
        KeyCode::F(4) => open_in_editor(files_view),
        KeyCode::Up => navigate(files_view, -1, rows_available),
        KeyCode::Down => navigate(files_view, 1, rows_available),
        KeyCode::Home => navigate_home(files_view),
        KeyCode::End => navigate_end(files_view, rows_available),
        KeyCode::Backspace => go_up_one_level(files_view),
        KeyCode::Enter => open_selected_file(files_view),
        KeyCode::Char('q') => {
            reset_terminal();
            std::process::exit(0);
        }
        _ => {}
    }
}

fn reset_filter_mode(files_view: &mut FilesView) {
    files_view.mode = FilewViewMode::Normal;
    files_view.filter_string.clear();
    update_files_view(files_view);
}

fn update_filter_string<F>(files_view: &mut FilesView, update_fn: F)
where
    F: FnOnce(&mut String),
{
    update_fn(&mut files_view.filter_string);
    update_files_view(files_view);
}

fn scroll_content(files_view: &mut FilesView, direction: isize) {
    let new_start = files_view.content_start as isize + direction;
    if new_start >= 0 && (new_start as usize) < files_view.content_lines.len() {
        files_view.content_start = new_start as usize;
    }
}

fn navigate(files_view: &mut FilesView, direction: isize, rows_available: u16) {
    let new_selected = files_view.selected as isize + direction;
    if new_selected >= 0 && (new_selected as usize) < files_view.files.len() {
        files_view.selected = new_selected as usize;
        if files_view.selected < files_view.start {
            files_view.start = files_view.selected;
        } else if files_view.selected >= files_view.start + rows_available as usize {
            files_view.start = files_view.selected - rows_available as usize + 1;
        }
    }
}

fn navigate_home(files_view: &mut FilesView) {
    files_view.selected = 0;
    files_view.start = 0;
}

fn navigate_end(files_view: &mut FilesView, rows_available: u16) {
    files_view.selected = files_view.files.len() - 1;
    files_view.start = if files_view.files.len() > rows_available as usize {
        files_view.files.len() - rows_available as usize
    } else {
        0
    };
}

fn open_selected_file(files_view: &mut FilesView) {
    let selected_file = files_view.files[files_view.selected].clone();
    if selected_file == ".." {
        go_up_one_level(files_view);
    } else if selected_file.ends_with('/') {
        change_directory(files_view, &selected_file);
    } else {
        open_file(files_view, &selected_file);
    }
}

fn change_directory(files_view: &mut FilesView, selected_file: &str) {
    files_view.mode = FilewViewMode::Normal;
    files_view.filter_string.clear();
    let separator_string = MAIN_SEPARATOR.to_string();
    let separator = if files_view.pwd.ends_with(MAIN_SEPARATOR) {
        ""
    } else {
        separator_string.as_str()
    };
    files_view.pwd = format!(
        "{}{}{}",
        files_view.pwd,
        separator,
        selected_file.trim_end_matches('/')
    );
    update_files_view(files_view);
}

fn open_file(files_view: &FilesView, selected_file: &str) {
    let file_path = format!("{}{}{}", files_view.pwd, MAIN_SEPARATOR, selected_file);
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

fn open_in_editor(files_view: &FilesView) {
    let selected_file = &files_view.files[files_view.selected];
    let file_path = format!("{}{}{}", files_view.pwd, MAIN_SEPARATOR, selected_file);

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

fn toggle_quick_view(files_view: &mut FilesView) {
    if files_view.mode == FilewViewMode::QuickView {
        files_view.mode = FilewViewMode::Normal;
    } else {
        let selected_file = &files_view.files[files_view.selected];
        let file_path = format!("{}{}{}", files_view.pwd, MAIN_SEPARATOR, selected_file);

        if !selected_file.ends_with('/') && fs::metadata(&file_path).is_ok() {
            if let Ok(content) = fs::read_to_string(&file_path) {
                files_view.content_lines = content.lines().map(String::from).collect();
                files_view.content_start = 0;
                files_view.mode = FilewViewMode::QuickView;
            } else {
                println!("Could not read file: {}", selected_file);
            }
        }
    }
}

fn go_up_one_level(files_view: &mut FilesView) {
    files_view.mode = FilewViewMode::Normal;
    files_view.filter_string.clear();
    let parent_dir = PathBuf::from(&files_view.pwd)
        .parent()
        .unwrap()
        .to_path_buf();
    files_view.pwd = parent_dir.to_str().unwrap().to_string();
    update_files_view(files_view);
}

fn update_files_view(files_view: &mut FilesView) {
    files_view.files = vec!["..".to_string()];
    files_view.files.extend(
        fs::read_dir(&files_view.pwd)
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
                files_view.filter_string.is_empty()
                    || name
                        .to_lowercase()
                        .contains(&files_view.filter_string.to_lowercase())
            })
            .collect::<Vec<String>>(),
    );
    files_view.selected = 0;
    files_view.start = 0;
}

fn perform_recursive_search(files_view: &mut FilesView) {
    let mut results = Vec::new();
    recursive_search(&files_view.pwd, &files_view.filter_string, &mut results);
    files_view.files = results;
}

fn recursive_search(dir: &str, query: &str, results: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
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
}

fn perform_ripgrep_search(files_view: &mut FilesView) {
    let mut results = Vec::new();
    ripgrep_search(&files_view.pwd, &files_view.filter_string, &mut results);
    files_view.files = results;
}

fn ripgrep_search(dir: &str, query: &str, results: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
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
}

fn draw_full_line(width: u16) {
    let text = (0..width)
        .map(|_| "─")
        .collect::<String>()
        .with(Color::Blue);
    println!("{}", text);
}
