use crossterm::{cursor, execute, queue, terminal};
use std::env;
use std::fs;
use std::io::{stdout, Write};

struct FilesView {
    files: Vec<String>,
    selected: usize,
    start: usize,
    pwd: String,
}

static HEADER_ROWS: u16 = 3;
static FOOTER_ROWS: u16 = 2;

fn main() {
    let mut stdout = stdout();
    let (columns, rows) = terminal::size().unwrap();

    let mut files_view = FilesView {
        files: vec![],
        selected: 0,
        start: 0,
        pwd: String::from(""),
    };

    files_view.pwd = env::current_dir().unwrap().to_str().unwrap().to_string();
    files_view.files = fs::read_dir(&files_view.pwd)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();

    let _ = execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::DisableBlinking
    );
    let _ = queue!(stdout, cursor::MoveTo(0, 0));

    // header
    println!("Hello, world!");
    println!("Terminal size: {}x{}", columns, rows);
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
        println!("{}", file);
    }

    // footer
    queue!(stdout, cursor::MoveTo(0, rows - FOOTER_ROWS)).unwrap();
    draw_full_line(columns);
    println!("Current directory: {}", &files_view.pwd);

    let _ = stdout.flush();
}

fn draw_full_line(width: u16) {
    let text = (0..width).map(|_| "-").collect::<String>();
    println!("{}", text);
}
