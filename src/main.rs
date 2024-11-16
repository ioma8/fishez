use crossterm::style::Stylize;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue, terminal,
};
use std::env;
use std::io::{stdout, Write};
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
}

static HEADER_ROWS: u16 = 2;
static FOOTER_ROWS: u16 = 2;

fn main() {
    let mut stdout = stdout();
    let (columns, rows) = terminal::size().unwrap();

    let mut files_view = FilesView {
        files: vec![],
        selected: 0,
        start: 0,
        pwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
        filter_string: String::new(),
        mode: FilewViewMode::Normal,
        content_lines: vec![],
        content_start: 0,
    };

    update_files_view(&mut files_view);

    loop {
        draw_ui(&mut stdout, &files_view, columns, rows);

        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) = event::read().unwrap()
        {
            if kind != event::KeyEventKind::Press {
                continue;
            }

            // Check for Ctrl+C
            if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
                break;
            }

            match files_view.mode {
                FilewViewMode::Normal => match code {
                    KeyCode::Char('s') => {
                        files_view.mode = FilewViewMode::Filter;
                    }
                    KeyCode::Esc => break,
                    _ => handle_navigation_keys(
                        &mut files_view,
                        code,
                        rows - HEADER_ROWS - FOOTER_ROWS,
                    ),
                },
                FilewViewMode::Filter => match code {
                    KeyCode::Esc => {
                        files_view.mode = FilewViewMode::Normal;
                        files_view.filter_string.clear();
                        update_files_view(&mut files_view);
                    }
                    KeyCode::Backspace => {
                        files_view.filter_string.pop();
                        update_files_view(&mut files_view);
                    }
                    KeyCode::Char(c) => {
                        files_view.filter_string.push(c);
                        update_files_view(&mut files_view);
                    }
                    _ => handle_navigation_keys(
                        &mut files_view,
                        code,
                        rows - HEADER_ROWS - FOOTER_ROWS,
                    ),
                },
                FilewViewMode::QuickView => match code {
                    KeyCode::Up => {
                        if files_view.content_start > 0 {
                            files_view.content_start -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if (files_view.content_start + (rows - HEADER_ROWS - FOOTER_ROWS) as usize)
                            < files_view.content_lines.len()
                        {
                            files_view.content_start += 1;
                        }
                    }
                    KeyCode::Esc => {
                        files_view.mode = FilewViewMode::Normal;
                    }
                    _ => {}
                },
            }
        }
    }

    // After exiting the loop, reset cursor settings and clear the terminal
    let _ = execute!(
        stdout,
        cursor::Show,
        cursor::EnableBlinking,
        terminal::Clear(terminal::ClearType::All)
    );
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
        draw_file_content(stdout, files_view, rows);
    } else {
        draw_files_list(files_view, rows);
    }

    draw_footer(stdout, files_view, columns, rows);

    let _ = stdout.flush();
}

fn draw_header(stdout: &mut std::io::Stdout, files_view: &FilesView, columns: u16) {
    if files_view.mode == FilewViewMode::QuickView {
        println!("Viewing: {}", &files_view.files[files_view.selected]);
    } else {
        println!("PWD: {}", files_view.pwd);
    }
    let title = "FISHEZ";
    let _ = queue!(stdout, cursor::MoveTo(columns - title.len() as u16, 0));
    println!("{}", title);
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

fn draw_file_content(stdout: &mut std::io::Stdout, files_view: &FilesView, rows: u16) {
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

    if files_view.mode == FilewViewMode::Filter {
        print!("Filter: {}", files_view.filter_string);
    } else {
        let total_dirs = files_view
            .files
            .iter()
            .filter(|name| name.ends_with('/'))
            .count();
        let total_files = files_view.files.len() - total_dirs - 1;
        print!("Total: {} dirs, {} files", total_dirs, total_files);
    }
}

fn handle_navigation_keys(files_view: &mut FilesView, code: KeyCode, rows_available: u16) {
    match files_view.mode {
        FilewViewMode::QuickView => match code {
            KeyCode::Up => {
                if files_view.content_start > 0 {
                    files_view.content_start -= 1;
                }
            }
            KeyCode::Down => {
                if (files_view.content_start + rows_available as usize)
                    < files_view.content_lines.len()
                {
                    files_view.content_start += 1;
                }
            }
            KeyCode::Esc => {
                files_view.mode = FilewViewMode::Normal;
            }
            _ => {}
        },
        _ => match code {
            KeyCode::Up => {
                if files_view.selected > 0 {
                    files_view.selected -= 1;
                    if files_view.selected < files_view.start {
                        files_view.start -= 1;
                    }
                }
            }
            KeyCode::Down => {
                if files_view.selected < files_view.files.len() - 1 {
                    files_view.selected += 1;
                    if files_view.selected >= files_view.start + rows_available as usize {
                        files_view.start += 1;
                    }
                }
            }
            KeyCode::Home => {
                files_view.selected = 0;
                files_view.start = 0;
            }
            KeyCode::End => {
                files_view.selected = files_view.files.len() - 1;
                files_view.start = if files_view.files.len() > rows_available as usize {
                    files_view.files.len() - rows_available as usize
                } else {
                    0
                };
            }
            KeyCode::Backspace => {
                go_up_one_level(files_view);
            }
            KeyCode::Enter => {
                let selected_file = &files_view.files[files_view.selected];
                if selected_file == ".." {
                    go_up_one_level(files_view);
                } else if selected_file.ends_with('/') {
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
                } else {
                    let file_path =
                        format!("{}{}{}", files_view.pwd, MAIN_SEPARATOR, selected_file);
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
            }
            KeyCode::F(3) => {
                if files_view.mode == FilewViewMode::QuickView {
                    files_view.mode = FilewViewMode::Normal;
                    return;
                } else {
                    files_view.mode = FilewViewMode::QuickView;
                }

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
            KeyCode::F(4) => {
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
            _ => {}
        },
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

fn draw_full_line(width: u16) {
    let text = (0..width).map(|_| "-").collect::<String>();
    println!("{}", text);
}
