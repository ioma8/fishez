use crossterm::style::{Color, Print, Stylize};
use crossterm::terminal::ClearType;
use crossterm::{cursor, queue, terminal};
use std::io::Write;

use crate::files_view::{FilesView, FilesViewMode, FOOTER_ROWS, HEADER_ROWS};

pub struct TerminalUI {
    pub columns: u16,
    pub rows: u16,
    stdout: std::io::Stdout,
}

impl TerminalUI {
    pub fn new() -> Self {
        let (columns, rows) = terminal::size().unwrap();
        TerminalUI {
            columns,
            rows,
            stdout: std::io::stdout(),
        }
    }

    pub fn draw_ui(&mut self, files_view: &FilesView) {
        let _ = queue!(&self.stdout, cursor::DisableBlinking, cursor::Hide);

        self.draw_header(files_view);

        if files_view.mode == FilesViewMode::QuickView {
            self.draw_file_content(files_view);
        } else {
            self.draw_files_list(files_view);
        }

        self.draw_footer(files_view);
        let _ = &self.stdout.flush();
    }

    fn draw_header(&self, files_view: &FilesView) {
        let _ = queue!(&self.stdout, cursor::MoveTo(0, 0));

        let left = if files_view.mode == FilesViewMode::QuickView {
            format!("Viewing: {}", &files_view.files[files_view.selected]).with(Color::Cyan)
        } else {
            format!("PWD: {}", files_view.pwd).with(Color::Cyan)
        };
        let right = "FISHEZ".with(Color::Cyan);
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            Print(left),
            terminal::Clear(ClearType::UntilNewLine),
            cursor::MoveTo(self.columns - right.content().len() as u16, 0),
            Print(right)
        );
        self.draw_full_line(1);
    }

    fn draw_files_list(&self, files_view: &FilesView) {
        let rows_available = self.rows - HEADER_ROWS - FOOTER_ROWS;
        let files_to_display = files_view.files[files_view.start..]
            .iter()
            .take(rows_available as usize);

        let _ = queue!(&self.stdout, cursor::MoveTo(0, HEADER_ROWS));

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

            let _ = queue!(
                &self.stdout,
                Print(name_final),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }

        let rows_to_clear = rows_available - files_view.files.len() as u16;
        for _ in 0..rows_to_clear {
            let _ = queue!(
                &self.stdout,
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
    }

    fn draw_file_content(&self, files_view: &FilesView) {
        let rows_available = self.rows - HEADER_ROWS - FOOTER_ROWS;

        let content_to_display = files_view.content_lines[files_view.content_start..]
            .iter()
            .take(rows_available as usize);

        let _ = queue!(&self.stdout, cursor::MoveTo(0, HEADER_ROWS));
        for line in content_to_display {
            let _ = queue!(
                &self.stdout,
                Print(line),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
        let rows_to_clear = rows_available - files_view.content_lines.len() as u16;
        for _ in 0..rows_to_clear {
            let _ = queue!(
                &self.stdout,
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
    }

    fn draw_footer(&self, files_view: &FilesView) {
        self.draw_full_line(self.rows - FOOTER_ROWS);
        let indicator_row = if files_view.mode == FilesViewMode::Filter
            || files_view.mode == FilesViewMode::RecursiveSearch
            || files_view.mode == FilesViewMode::RipGrep
        {
            let filter_name = match files_view.mode {
                FilesViewMode::Filter => "Filter",
                FilesViewMode::RecursiveSearch => "Search",
                FilesViewMode::RipGrep => "RipGrep",
                _ => "",
            };
            format!("{}: {}", filter_name, files_view.filter_string).with(Color::Green)
        } else {
            let total_dirs = files_view
                .files
                .iter()
                .filter(|name| name.ends_with('/'))
                .count();
            let total_files = files_view.files.len() - total_dirs - 1;
            format!("{} dirs, {} files", total_dirs, total_files).with(Color::Green)
        };
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
        let actions_row = actions.join(&" ".repeat(padding)).with(Color::Green);

        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, self.rows - FOOTER_ROWS + 1),
            Print(indicator_row),
            cursor::MoveTo(0, self.rows - FOOTER_ROWS + 2),
            Print(actions_row)
        );
    }

    fn draw_full_line(&self, row: u16) {
        let text = (0..self.columns)
            .map(|_| "─")
            .collect::<String>()
            .with(Color::Blue);
        let _ = queue!(&self.stdout, cursor::MoveTo(0, row), Print(text),);
    }
}
