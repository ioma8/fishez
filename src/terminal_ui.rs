use crossterm::style::{Color, Print, Stylize};
use crossterm::terminal::ClearType;
use crossterm::{cursor, queue, terminal};
use image::{self, ImageBuffer};
use std::io::Write;
use std::time::Instant;

use crate::files_view::{FilesView, FilesViewMode, QuickViewMode, FOOTER_ROWS, HEADER_ROWS};
use crate::logger::log;

pub struct TerminalUI {
    pub columns: u16,
    pub rows: u16,
    stdout: std::io::Stdout,
}

impl TerminalUI {
    pub fn new() -> Self {
        let (columns, rows) = terminal::size().expect("Error getting terminal size");
        TerminalUI {
            columns,
            rows,
            stdout: std::io::stdout(),
        }
    }

    pub fn draw_ui(&mut self, files_view: &FilesView) {
        let _ = queue!(&self.stdout, cursor::DisableBlinking, cursor::Hide);

        self.draw_header(files_view);
        log(&format!("mode: {:?}", files_view.mode));

        match &files_view.mode {
            FilesViewMode::Normal | FilesViewMode::Filter => self.draw_files_list(files_view),
            FilesViewMode::QuickView(quick_view) => self.draw_file_content(quick_view),
            _ => {}
        }

        self.draw_footer(files_view);
        let _ = &self.stdout.flush();
    }

    fn draw_header(&self, files_view: &FilesView) {
        let _ = queue!(&self.stdout, cursor::MoveTo(0, 0));

        let left = if let FilesViewMode::QuickView(_) = files_view.mode {
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

    fn draw_files_list(&mut self, files_view: &FilesView) {
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

        let rows_to_clear: i16 = rows_available as i16 - files_view.files.len() as i16;

        if rows_to_clear > 0 {
            for _ in 0..rows_to_clear {
                let _ = queue!(
                    &self.stdout,
                    terminal::Clear(ClearType::UntilNewLine),
                    cursor::MoveToNextLine(1)
                );
            }
        }
    }

    fn draw_file_content(&mut self, quick_view: &QuickViewMode) {
        let rows_available = self.rows - HEADER_ROWS - FOOTER_ROWS;
        log(&format!("rows_available: {}", rows_available));
        match quick_view {
            QuickViewMode::Text(content, start, _) => {
                self.draw_text_content(content, *start, rows_available);
            }
            QuickViewMode::Image(data) => {
                self.draw_image_content(data.clone());
            }
            _ => {
                self.draw_text_content(&vec!["".to_string()], 0, rows_available);
            }
        }
    }

    fn draw_text_content(&self, content: &Vec<String>, start: usize, rows_available: u16) {
        let content_to_display = content[start..].iter().take(rows_available as usize);
        log(&format!("content_to_display: {:?}", content_to_display));
        let _ = queue!(&self.stdout, cursor::MoveTo(0, HEADER_ROWS));
        for line in content_to_display {
            let _ = queue!(
                &self.stdout,
                Print(line),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
        let rows_to_clear: i16 = rows_available as i16 - content.len() as i16;
        if rows_to_clear > 0 {
            for _ in 0..rows_to_clear {
                let _ = queue!(
                    &self.stdout,
                    terminal::Clear(ClearType::UntilNewLine),
                    cursor::MoveToNextLine(1)
                );
            }
        }
    }

    fn draw_image_content(&mut self, data: ImageBuffer<image::Rgb<u8>, Vec<u8>>) {
        let now = Instant::now();
        let (orig_width, orig_height) = data.dimensions();
        let (new_width, new_height) = self.calculate_aspect_ratio_fit(orig_width * 2, orig_height);
        let duration = now.elapsed();
        log(&format!("calculate_aspect_ratio_fit {:?}", duration));

        let now = Instant::now();
        let resized = image::imageops::resize(
            &data,
            new_width,
            new_height,
            image::imageops::FilterType::Nearest,
        );
        let duration = now.elapsed();
        log(&format!("resize {:?}", duration));

        let rem_horizontal_padding = (self.columns - new_width as u16) / 2;

        let now = Instant::now();
        let mut i = 0;
        for pixel in resized.pixels() {
            if i == 0 {
                let _ = queue!(
                    &self.stdout,
                    Print(" ".repeat(rem_horizontal_padding as usize))
                );
            }
            let [r, g, b] = pixel.0;
            let color: Color = Color::Rgb { r, g, b };
            let _ = queue!(&self.stdout, Print(" ".with(color).on(color)));
            i += 1;
            if i == new_width {
                let _ = queue!(
                    &self.stdout,
                    terminal::Clear(ClearType::UntilNewLine),
                    cursor::MoveToNextLine(1)
                );
                i = 0;
            }
        }
        let duration = now.elapsed();
        log(&format!("draw {:?}", duration));
    }

    fn calculate_aspect_ratio_fit(&self, orig_width: u32, orig_height: u32) -> (u32, u32) {
        let max_width = self.columns as u32;
        let max_height = (self.rows - HEADER_ROWS - FOOTER_ROWS) as u32;
        let aspect_ratio = orig_width as f32 / orig_height as f32;

        if max_width as f32 / aspect_ratio <= max_height as f32 {
            (max_width, (max_width as f32 / aspect_ratio) as u32)
        } else {
            ((max_height as f32 * aspect_ratio) as u32, max_height)
        }
    }

    fn get_footer_actions_by_mode(&self, mode: &FilesViewMode) -> Vec<&str> {
        match mode {
            FilesViewMode::Normal => vec![
                "[f3]view",
                "[f4]edit",
                "[f6]recursive",
                "[f7]ripgrep",
                "[f10]quit",
            ],
            FilesViewMode::Filter => vec!["[f3]view", "[f4]edit", "[esc]clear", "[f10]quit"],
            FilesViewMode::QuickView(_) => vec![
                "[up]scroll up",
                "[down]scroll down",
                "[left]previous file",
                "[right]next file",
                "[f3]close view",
            ],
            FilesViewMode::RecursiveSearch => vec!["[esc]close search", "[f10]quit"],
            FilesViewMode::RipGrep => vec!["[esc]close ripgrep", "[f10]quit"],
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
        } else if matches!(files_view.mode, FilesViewMode::QuickView(_)) {
            format!(
                "File {} / {}",
                files_view.selected + 1,
                files_view.files.len()
            )
            .with(Color::Green)
        } else {
            let total_dirs = files_view
                .files
                .iter()
                .filter(|name| name.ends_with('/'))
                .count();
            let total_files = files_view.files.len() - total_dirs - 1;
            format!("{} dirs, {} files", total_dirs, total_files).with(Color::Green)
        };
        let actions = self.get_footer_actions_by_mode(&files_view.mode);
        let actions_str = actions.join(" ");
        let padding = (self.columns as usize - actions_str.len()) / (actions.len() - 1);
        let actions_row = actions.join(&" ".repeat(padding)).with(Color::Green);

        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, self.rows - FOOTER_ROWS + 1),
            Print(indicator_row),
            terminal::Clear(ClearType::UntilNewLine),
            cursor::MoveTo(0, self.rows - FOOTER_ROWS + 2),
            Print(actions_row),
            terminal::Clear(ClearType::UntilNewLine),
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
