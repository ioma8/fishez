use crossterm::event::KeyEvent;
use crossterm::style::{Color, Print, StyledContent, Stylize};
use crossterm::terminal::{ClearType, enable_raw_mode};
use crossterm::{cursor, queue, terminal};
use image::{self, ImageBuffer};
use std::io::Write;
use std::path::MAIN_SEPARATOR;
use std::sync::mpsc::Sender;
use std::vec;

use crate::files_view::{FOOTER_ROWS, FilesView, FilesViewMode, HEADER_ROWS, QuickViewMode};

pub trait FeatureTrait {
    fn captured_key_event(
        &mut self,
        _event: KeyEvent,
        _files_view: &mut FilesView,
        _sender: &Sender<Message>,
    ) -> bool {
        false
    }
    fn drawn_header(&self, _files_view: &FilesView, _terminal_ui: &TerminalUI) -> bool {
        false
    }
    fn drawn_content(&self, _files_view: &FilesView, _terminal_ui: &TerminalUI) -> bool {
        false
    }
    fn drawn_footer(&self, _files_view: &FilesView, _terminal_ui: &TerminalUI) -> bool {
        false
    }
    fn modify_footer_actions(&self, _actions: &mut Vec<&str>) {}
    fn map_item(
        &self,
        item: StyledContent<String>,
        _index: usize,
        _files_view: &FilesView,
    ) -> StyledContent<String> {
        item
    }
}

pub struct TerminalUI {
    pub columns: u16,
    pub rows: u16,
    features: Vec<Box<dyn FeatureTrait>>,
    pub stdout: std::io::Stdout,
    pub sender: Sender<Message>,
}

#[derive(Debug)]
pub enum Message {
    DrawFiles(Vec<String>),
}

#[derive(Clone, Copy, Debug)]
pub enum ActivePane {
    Left,
    Right,
}

impl TerminalUI {
    pub fn new(sender: Sender<Message>) -> Self {
        let (columns, rows) = terminal::size().expect("Error getting terminal size");
        enable_raw_mode().expect("Failed to enable raw mode");

        TerminalUI {
            columns,
            rows,
            stdout: std::io::stdout(),
            features: vec![],
            sender,
        }
    }

    pub fn add_feature(&mut self, feature: Box<dyn FeatureTrait>) {
        self.features.push(feature);
    }

    pub fn handle_features_shortcuts(
        &mut self,
        event: KeyEvent,
        files_view: &mut FilesView,
    ) -> bool {
        for feature in self.features.iter_mut() {
            if feature.captured_key_event(event, files_view, &self.sender) {
                return true;
            }
        }

        false
    }

    fn draw_feature_header(&self, files_view: &FilesView) -> bool {
        for feature in self.features.iter() {
            if feature.drawn_header(files_view, self) {
                return true;
            }
        }
        false
    }

    fn draw_feature_footer(&self, files_view: &FilesView) -> bool {
        for feature in self.features.iter() {
            if feature.drawn_footer(files_view, self) {
                return true;
            }
        }
        false
    }

    fn draw_feature_content(&self, files_view: &FilesView) -> bool {
        for feature in self.features.iter() {
            if feature.drawn_content(files_view, self) {
                return true;
            }
        }
        false
    }

    pub fn draw_ui(&mut self, files_view: &mut FilesView) {
        files_view.clear_notification();
        let _ = queue!(&self.stdout, cursor::DisableBlinking, cursor::Hide);

        if !self.draw_feature_header(files_view) {
            self.draw_default_header(files_view);
        }
        self.draw_system_header(files_view);

        if !self.draw_feature_content(files_view) {
            match &files_view.mode {
                FilesViewMode::Normal | FilesViewMode::Filter => self.draw_files_list(files_view),
                FilesViewMode::QuickView(quick_view) => self.draw_file_content(quick_view),
            }
        }

        if !self.draw_feature_footer(files_view) {
            self.draw_default_footer(files_view);
        }

        self.draw_footer_actions(files_view);

        let _ = &self.stdout.flush();
    }

    pub fn draw_ui_two_panes(
        &mut self,
        left: &mut FilesView,
        right: &mut FilesView,
        active: ActivePane,
    ) {
        left.clear_notification();
        right.clear_notification();
        let _ = queue!(&self.stdout, cursor::DisableBlinking, cursor::Hide);

        // QuickView takes full screen for the active pane to keep changes minimal.
        let active_is_quickview = match active {
            ActivePane::Left => matches!(left.mode, FilesViewMode::QuickView(_)),
            ActivePane::Right => matches!(right.mode, FilesViewMode::QuickView(_)),
        };
        if active_is_quickview {
            let target = match active {
                ActivePane::Left => left,
                ActivePane::Right => right,
            };
            self.draw_ui(target);
            return;
        }

        self.draw_dual_header(left, right, active);
        let active_for_header: &FilesView = match active {
            ActivePane::Left => left,
            ActivePane::Right => right,
        };
        self.draw_system_header(active_for_header);
        self.draw_files_list_two_panes(left, right, active);
        let active_for_footer: &FilesView = match active {
            ActivePane::Left => left,
            ActivePane::Right => right,
        };
        self.draw_default_footer(active_for_footer);
        self.draw_footer_actions(active_for_footer);
        let _ = &self.stdout.flush();
    }

    fn draw_default_header(&self, files_view: &FilesView) {
        let _ = queue!(&self.stdout, cursor::MoveTo(0, 0));

        let left = if let FilesViewMode::QuickView(_) = files_view.mode {
            format!("Viewing: {}", &files_view.files[files_view.selected]).with(Color::Cyan)
        } else {
            format!("PWD: {}", files_view.pwd).with(Color::Cyan)
        };
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            Print(left),
            terminal::Clear(ClearType::UntilNewLine),
        );
    }

    fn draw_dual_header(&self, left: &FilesView, right: &FilesView, active: ActivePane) {
        let _ = queue!(&self.stdout, cursor::MoveTo(0, 0), terminal::Clear(ClearType::UntilNewLine));
        let pane_width = self.columns.saturating_sub(1) / 2;

        let (left_label, right_label) = match active {
            ActivePane::Left => ("*L", " R"),
            ActivePane::Right => (" L", "*R"),
        };

        let left_text = format!("{}: {}", left_label, left.pwd);
        let right_text = format!("{}: {}", right_label, right.pwd);

        let truncated_left = self.truncate_plain(&left_text, pane_width as usize);
        let truncated_right = self.truncate_plain(&right_text, pane_width as usize);

        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            Print(truncated_left.with(Color::Cyan)),
            cursor::MoveTo(pane_width + 1, 0),
            Print(truncated_right.with(Color::Cyan)),
        );
    }

    fn draw_system_header(&self, files_view: &FilesView) {
        let right = if let Some(notification) = files_view.notification.clone() {
            notification.on(Color::DarkMagenta)
        } else {
            "FISHEZ".to_string().with(Color::Cyan)
        };
        let _ = queue!(
            &self.stdout,
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
            let name = if file.ends_with(MAIN_SEPARATOR) {
                file.to_string().yellow()
            } else {
                file.to_string().dark_yellow()
            };

            let name_final = if i == files_view.selected - files_view.start {
                name.negative()
            } else {
                name
            };

            let mut name_after_features = name_final;

            for feature in &self.features {
                name_after_features =
                    feature.map_item(name_after_features, i + files_view.start, files_view);
            }

            let _ = queue!(
                &self.stdout,
                Print(name_after_features),
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

        // TODO: udelat z toho obecnou metodu pro kresleni scrollbaru ze zobrazenych polozek
        // Calculate scrollbar parameters
        let total_items = files_view.files.len();
        let visible_items = rows_available as usize;
        if total_items > visible_items {
            let scrollbar_height = (visible_items as f32 / total_items as f32
                * visible_items as f32)
                .round()
                .max(1.0) as u16;

            let scrollbar_position = (files_view.start as f32 / total_items as f32
                * visible_items as f32)
                .round()
                .max(0.0) as u16;

            // Draw the scrollbar
            for i in 0..rows_available {
                if i >= scrollbar_position && i < scrollbar_position + scrollbar_height {
                    // Filled portion of the scrollbar
                    let _ = queue!(
                        &self.stdout,
                        cursor::MoveTo(self.columns - 1, HEADER_ROWS + i),
                        Print("|".dark_blue())
                    );
                }
            }
        }
    }

    fn styled_name_for_index(
        &self,
        files_view: &FilesView,
        index: usize,
    ) -> StyledContent<String> {
        let file = &files_view.files[index];
        let name = if file.ends_with(MAIN_SEPARATOR) {
            file.to_string().yellow()
        } else {
            file.to_string().dark_yellow()
        };

        let name_final = if index == files_view.selected {
            name.negative()
        } else {
            name
        };

        let mut name_after_features = name_final;

        for feature in &self.features {
            name_after_features = feature.map_item(name_after_features, index, files_view);
        }

        name_after_features
    }

    fn truncate_styled(&self, item: StyledContent<String>, width: u16) -> StyledContent<String> {
        let text = item.content().clone();
        if text.len() as u16 > width && width > 1 {
            let mut truncated = text
                .chars()
                .take((width - 1) as usize)
                .collect::<String>();
            truncated.push('…');
            item.style().apply(truncated)
        } else {
            item
        }
    }

    fn truncate_plain(&self, text: &str, width: usize) -> String {
        if text.len() > width && width > 1 {
            let mut truncated = text.chars().take(width - 1).collect::<String>();
            truncated.push('…');
            truncated
        } else {
            text.to_string()
        }
    }

    fn draw_files_list_two_panes(
        &mut self,
        left: &FilesView,
        right: &FilesView,
        active: ActivePane,
    ) {
        let rows_available = self.rows - HEADER_ROWS - FOOTER_ROWS;
        let pane_width = self.columns.saturating_sub(1) / 2;
        let separator_col = pane_width;
        let right_col = separator_col + 1;

        for row in 0..rows_available {
            let screen_row = HEADER_ROWS + row;
            let _ = queue!(
                &self.stdout,
                cursor::MoveTo(0, screen_row),
                terminal::Clear(ClearType::UntilNewLine)
            );

            let left_index = left.start + row as usize;
            if left_index < left.files.len() {
                let left_name = self.styled_name_for_index(left, left_index);
                let left_trunc = self.truncate_styled(left_name, pane_width);
                let _ = queue!(&self.stdout, cursor::MoveTo(0, screen_row), Print(left_trunc));
            }

            // separator
            let sep_color = if matches!(active, ActivePane::Left) {
                Color::Blue
            } else {
                Color::Grey
            };
            let _ = queue!(
                &self.stdout,
                cursor::MoveTo(separator_col, screen_row),
                Print("│".with(sep_color))
            );

            let right_index = right.start + row as usize;
            if right_index < right.files.len() {
                let right_name = self.styled_name_for_index(right, right_index);
                let right_trunc = self.truncate_styled(right_name, pane_width.saturating_sub(1));
                let _ = queue!(&self.stdout, cursor::MoveTo(right_col, screen_row), Print(right_trunc));
            }
        }
    }

    fn draw_file_content(&mut self, quick_view: &QuickViewMode) {
        let rows_available = self.rows - HEADER_ROWS - FOOTER_ROWS;
        match quick_view {
            QuickViewMode::Text {
                lines: content,
                start,
                length: _,
            } => {
                self.draw_text_content(content, *start, rows_available);
            }
            QuickViewMode::Image(data, bytes) => {
                let encoded = iterm2img::from_bytes(bytes.to_vec())
                    .width(self.columns as u64)
                    .height(rows_available as u64)
                    .width_auto()
                    .preserve_aspect_ratio(true)
                    .inline(true)
                    .build();
                let _ = queue!(
                    &self.stdout,
                    cursor::MoveTo(0, HEADER_ROWS),
                    Print(encoded),
                    terminal::Clear(ClearType::UntilNewLine)
                );
                // TODO: add switch to this
                //self.draw_image_content(data.clone());
            }
            QuickViewMode::Directory { lines } => {
                self.draw_text_content(lines, 0, rows_available);
            }
            _ => {
                self.draw_text_content(&vec!["".into()], 0, rows_available);
            }
        }
    }

    fn draw_text_content(&self, content: &Vec<String>, start: usize, rows_available: u16) {
        let content_to_display = content[start..].iter().take(rows_available as usize);
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
        let (orig_width, orig_height) = data.dimensions();
        let (new_width, new_height) = self.calculate_aspect_ratio_fit(orig_width * 2, orig_height);

        let resized = image::imageops::resize(
            &data,
            new_width,
            new_height,
            image::imageops::FilterType::Nearest,
        );

        let rem_horizontal_padding = (self.columns - new_width as u16) / 2;

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
            FilesViewMode::Normal => vec!["[f3]view"],
            FilesViewMode::Filter => vec!["[f3]view", "[f4]edit", "[esc]clear"],
            FilesViewMode::QuickView(_) => vec![
                "[up]scroll up",
                "[down]scroll down",
                "[left]previous file",
                "[right]next file",
                "[f3]close view",
            ],
        }
    }

    fn draw_default_footer(&self, files_view: &FilesView) {
        self.draw_full_line(self.rows - FOOTER_ROWS);
        let indicator_row = if files_view.mode == FilesViewMode::Filter {
            let filter_name = match files_view.mode {
                FilesViewMode::Filter => "Filter",
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
                .filter(|name| name.ends_with(MAIN_SEPARATOR))
                .count();
            let total_files = files_view.files.len() - total_dirs - 1;
            format!("{} dirs, {} files", total_dirs, total_files).with(Color::Green)
        };

        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, self.rows - FOOTER_ROWS + 1),
            Print(indicator_row),
            terminal::Clear(ClearType::UntilNewLine),
        );
    }

    fn draw_footer_actions(&self, files_view: &FilesView) {
        let mut actions = self.get_footer_actions_by_mode(&files_view.mode);

        // TODO: rewrite quickview to be a feature
        if !matches!(&files_view.mode, FilesViewMode::QuickView(_)) {
            self.features.iter().for_each(|feature| {
                feature.modify_footer_actions(&mut actions);
            });
        }
        actions.push("[f10]quit");

        let actions_str = actions.join("");
        // Avoid underflow or division by zero on narrow terminals.
        let available = self.columns as usize;
        let padding = if actions.len() > 1 && available > actions_str.len() {
            (available - actions_str.len()) / (actions.len() - 1)
        } else {
            1
        };
        let actions_row = actions.join(&" ".repeat(padding.max(1))).with(Color::Green);

        let _ = queue!(
            &self.stdout,
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

    pub fn reset_terminal(&mut self) {
        println!("Dropping TerminalUI, restoring terminal state...");
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            cursor::Show,
            cursor::EnableBlinking,
            terminal::Clear(ClearType::All)
        );
        let _ = &self.stdout.flush();
    }
}

impl Drop for TerminalUI {
    fn drop(&mut self) {
        self.reset_terminal();
    }
}
