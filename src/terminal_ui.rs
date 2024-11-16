use crossterm::style::{Color, Stylize};
use crossterm::{cursor, execute, queue, terminal};
use std::io::Write;

use crate::files_view::{FilesView, FilesViewMode, FOOTER_ROWS, HEADER_ROWS};

pub struct TerminalUI {
    pub columns: u16,
    pub rows: u16,
}

impl TerminalUI {
    pub fn new() -> Self {
        let (columns, rows) = terminal::size().unwrap();
        TerminalUI { columns, rows }
    }

    pub fn draw_ui(&self, stdout: &mut std::io::Stdout, files_view: &FilesView) {
        let _ = execute!(
            stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::DisableBlinking,
            cursor::Hide
        );
        let _ = queue!(stdout, cursor::MoveTo(0, 0));

        self.draw_header(stdout, files_view);

        if files_view.mode == FilesViewMode::QuickView {
            self.draw_file_content(files_view);
        } else {
            self.draw_files_list(files_view);
        }

        self.draw_footer(stdout, files_view);
        let _ = stdout.flush();
    }

    fn draw_header(&self, stdout: &mut std::io::Stdout, files_view: &FilesView) {
        if files_view.mode == FilesViewMode::QuickView {
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

        if files_view.mode == FilesViewMode::Filter
            || files_view.mode == FilesViewMode::RecursiveSearch
            || files_view.mode == FilesViewMode::RipGrep
        {
            let filter_name = match files_view.mode {
                FilesViewMode::Filter => "Filter",
                FilesViewMode::RecursiveSearch => "Search",
                FilesViewMode::RipGrep => "RipGrep",
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
