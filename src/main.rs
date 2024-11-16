use crossterm::style::Stylize;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
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
}

#[derive(PartialEq)]
enum FilewViewMode {
    Normal,
    Filter,
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
    };

    update_files_view(&mut files_view);

    loop {
        let _ = execute!(
            stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::DisableBlinking,
            cursor::Hide
        );
        let _ = queue!(stdout, cursor::MoveTo(0, 0));

        // header
        print!("PWD: {}", &files_view.pwd);
        let title = "FISHEZ";
        let _ = queue!(stdout, cursor::MoveTo(columns - title.len() as u16, 0));
        println!("{}", title);
        draw_full_line(columns);

        // files list view (scrollable)
        let rows_available = rows - HEADER_ROWS - FOOTER_ROWS;
        let files_to_display = files_view.files[files_view.start..]
            .iter()
            .take(rows_available as usize)
            .collect::<Vec<&String>>();
        for (i, file) in files_to_display.iter().enumerate() {
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

        // footer
        queue!(stdout, cursor::MoveTo(0, rows - FOOTER_ROWS)).unwrap();
        draw_full_line(columns);
        if files_view.mode == FilewViewMode::Filter {
            print!("Filter: {}", files_view.filter_string);
        } else {
            print!("{} files", files_view.files.len());
        }

        let _ = stdout.flush();

        if let Event::Key(KeyEvent { code, kind, .. }) = event::read().unwrap() {
            if kind != event::KeyEventKind::Press {
                continue;
            }
            match files_view.mode {
                FilewViewMode::Normal => match code {
                    KeyCode::Char('s') => {
                        files_view.mode = FilewViewMode::Filter;
                    }
                    KeyCode::Esc => break,
                    _ => handle_navigation_keys(&mut files_view, code, rows_available),
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
                    _ => handle_navigation_keys(&mut files_view, code, rows_available),
                },
            }
        }
    }
}

fn handle_navigation_keys(files_view: &mut FilesView, code: KeyCode, rows_available: u16) {
    match code {
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
            files_view.mode = FilewViewMode::Normal;
            files_view.filter_string.clear();
            go_up_one_level(files_view);
            update_files_view(files_view);
        }
        KeyCode::Enter => {
            files_view.mode = FilewViewMode::Normal;
            files_view.filter_string.clear();
            let selected_file = &files_view.files[files_view.selected];
            if selected_file == ".." {
                go_up_one_level(files_view);
            } else if selected_file.ends_with('/') {
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
                    selected_file.trim_end_matches("/")
                );
            } else {
                let file_path = format!("{}{}{}", files_view.pwd, MAIN_SEPARATOR, selected_file);
                if cfg!(target_os = "windows") {
                    Command::new("cmd")
                        .args(&["/C", "start", "", &file_path])
                        .spawn()
                        .unwrap();
                } else if cfg!(target_os = "macos") {
                    Command::new("open").arg(&file_path).spawn().unwrap();
                } else {
                    Command::new("xdg-open").arg(&file_path).spawn().unwrap();
                }
            }
            update_files_view(files_view);
        }
        _ => {}
    }
}

fn go_up_one_level(files_view: &mut FilesView) {
    let parent_dir = PathBuf::from(&files_view.pwd)
        .parent()
        .unwrap()
        .to_path_buf();
    files_view.pwd = parent_dir.to_str().unwrap().to_string();
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
