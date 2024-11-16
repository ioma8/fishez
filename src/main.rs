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
        let mut filter_string = String::new();

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
            if i == files_view.selected {
                print!("> ");
            } else {
                print!("  ");
            }
            if file.ends_with('/') {
                println!("{}", file.as_str().yellow())
            } else {
                println!("{}", file.as_str().dark_yellow());
            }
        }

        // footer
        queue!(stdout, cursor::MoveTo(0, rows - FOOTER_ROWS)).unwrap();
        draw_full_line(columns);
        print!("Filter: {}", filter_string);

        let _ = stdout.flush();

        if let Event::Key(KeyEvent { code, kind, .. }) = event::read().unwrap() {
            if kind != event::KeyEventKind::Press {
                continue;
            }
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
                KeyCode::Backspace => {
                    go_up_one_level(&mut files_view);
                    update_files_view(&mut files_view);
                }
                KeyCode::Enter => {
                    let selected_file = &files_view.files[files_view.selected];
                    if selected_file == ".." {
                        go_up_one_level(&mut files_view);
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
                        let file_path =
                            format!("{}{}{}", files_view.pwd, MAIN_SEPARATOR, selected_file);
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
                    update_files_view(&mut files_view);
                }
                KeyCode::Esc => break,
                _ => {}
            }
        }
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
            .collect::<Vec<String>>(),
    );
    files_view.selected = 0;
    files_view.start = 0;
}

fn draw_full_line(width: u16) {
    let text = (0..width).map(|_| "-").collect::<String>();
    println!("{}", text);
}
