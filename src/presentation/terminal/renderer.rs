//! Terminal renderer - handles all drawing to the terminal.

use crate::application::{ActivePane, AppState, PanelMode, PanelState, QuickViewMode};
use crate::domain::FileEntry;
use crossterm::style::{Color, Print, StyledContent, Stylize};
use crossterm::terminal::{ClearType, enable_raw_mode};
use crossterm::{cursor, queue, terminal};
use std::io::Write;

pub const HEADER_ROWS: u16 = 2;
pub const FOOTER_ROWS: u16 = 3;
const HELP_OVERLAY_START_ROW: u16 = 3;
const HELP_OVERLAY_COL_GAP: usize = 4;

/// Presentation message for async operations.
#[derive(Debug)]
#[allow(dead_code)]
pub enum Message {
    DrawFiles(Vec<String>),
}

#[derive(Debug, Clone)]
pub enum FooterActionsPosition {
    Start,
    End,
    Nondetermined,
}

/// Terminal renderer - draws the UI based on AppState.
pub struct TerminalRenderer {
    pub columns: u16,
    pub rows: u16,
    pub stdout: std::io::Stdout,
    help_entries: Vec<(String, String)>,
}

impl Default for TerminalRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalRenderer {
    pub fn new() -> Self {
        let (columns, rows) = terminal::size().expect("Error getting terminal size");
        enable_raw_mode().expect("Failed to enable raw mode");

        Self {
            columns,
            rows,
            stdout: std::io::stdout(),
            help_entries: Self::default_help_entries(),
        }
    }

    fn default_help_entries() -> Vec<(String, String)> {
        vec![
            ("F1".into(), "Toggle help overlay".into()),
            ("Arrows".into(), "Navigate".into()),
            ("Enter".into(), "Open dir/file".into()),
            ("Backspace".into(), "Go up one level".into()),
            ("F3".into(), "Quick view (text/images/dirs)".into()),
            ("Tab".into(), "Switch pane (two-pane)".into()),
            ("Esc".into(), "Cancel filter/close view".into()),
            ("F10/Ctrl+C".into(), "Quit".into()),
            ("Ctrl+W".into(), "Delete selected/current to trash".into()),
            ("F6".into(), "Find (fd)".into()),
            ("F7".into(), "RipGrep search".into()),
            ("Ctrl+D".into(), "Favorites list / add current dir".into()),
            ("Space".into(), "Toggle selection for batch actions".into()),
            ("F4".into(), "Open in VS Code".into()),
        ]
    }

    /// Updates the terminal size.
    pub fn update_size(&mut self, cols: u16, rows: u16) {
        self.columns = cols;
        self.rows = rows;
    }

    /// Draws the UI for single pane mode.
    pub fn draw(&mut self, state: &AppState) {
        let panel = state.active_panel();
        let _ = queue!(&self.stdout, cursor::DisableBlinking, cursor::Hide);

        self.draw_header(panel);
        self.draw_system_header(panel);

        match &panel.mode {
            PanelMode::Normal | PanelMode::Filter => self.draw_files_list(panel),
            PanelMode::QuickView(qv) => self.draw_quick_view(qv),
        }

        self.draw_footer(panel);
        self.draw_footer_actions(&panel.mode);

        if state.show_help {
            self.draw_help_overlay();
        }

        let _ = self.stdout.flush();
    }

    /// Draws the UI for two pane mode.
    pub fn draw_two_panes(&mut self, state: &AppState) {
        let _ = queue!(&self.stdout, cursor::DisableBlinking, cursor::Hide);

        // QuickView takes full screen for the active pane
        let active_is_quickview = matches!(state.active_panel().mode, PanelMode::QuickView(_));
        if active_is_quickview {
            self.draw(state);
            return;
        }

        self.draw_dual_header(&state.left_panel, &state.right_panel, state.active_pane);
        self.draw_system_header(state.active_panel());
        self.draw_files_list_two_panes(&state.left_panel, &state.right_panel, state.active_pane);
        self.draw_footer(state.active_panel());
        self.draw_footer_actions(&state.active_panel().mode);

        if state.show_help {
            self.draw_help_overlay();
        }

        let _ = self.stdout.flush();
    }

    fn draw_header(&self, panel: &PanelState) {
        let _ = queue!(&self.stdout, cursor::MoveTo(0, 0));

        let left = if let PanelMode::QuickView(_) = panel.mode {
            if panel.cursor < panel.entries.len() {
                format!("Viewing: {}", &panel.entries[panel.cursor].name).with(Color::Cyan)
            } else {
                "Viewing".to_string().with(Color::Cyan)
            }
        } else {
            format!("PWD: {}", panel.current_path.display()).with(Color::Cyan)
        };

        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            Print(left),
            terminal::Clear(ClearType::UntilNewLine),
        );
    }

    fn draw_dual_header(&self, left: &PanelState, right: &PanelState, active: ActivePane) {
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            terminal::Clear(ClearType::UntilNewLine)
        );
        let pane_width = self.columns.saturating_sub(1) / 2;

        let (left_label, right_label) = match active {
            ActivePane::Left => ("*L", " R"),
            ActivePane::Right => (" L", "*R"),
        };

        let left_text = format!("{}: {}", left_label, left.current_path.display());
        let right_text = format!("{}: {}", right_label, right.current_path.display());

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

    fn draw_system_header(&self, panel: &PanelState) {
        let right = if let Some(notification) = panel.notification.clone() {
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

    fn draw_files_list(&mut self, panel: &PanelState) {
        let rows_available = self.rows - HEADER_ROWS - FOOTER_ROWS;
        let entries_to_display = panel.entries[panel.scroll..]
            .iter()
            .take(rows_available as usize);

        let _ = queue!(&self.stdout, cursor::MoveTo(0, HEADER_ROWS));

        for (i, entry) in entries_to_display.enumerate() {
            let name = self.style_entry_name(entry);
            let name_final = if i == panel.cursor - panel.scroll {
                name.negative()
            } else {
                name
            };

            let name_after_features = if panel.is_multi_selected(i + panel.scroll) {
                name_final.on(Color::Blue)
            } else {
                name_final
            };

            let _ = queue!(
                &self.stdout,
                Print(name_after_features),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }

        let rows_to_clear: i16 = rows_available as i16 - panel.entries.len() as i16;
        if rows_to_clear > 0 {
            for _ in 0..rows_to_clear {
                let _ = queue!(
                    &self.stdout,
                    terminal::Clear(ClearType::UntilNewLine),
                    cursor::MoveToNextLine(1)
                );
            }
        }

        // Draw scrollbar
        let total_items = panel.entries.len();
        let visible_items = rows_available as usize;
        if total_items > visible_items {
            let scrollbar_height = (visible_items as f32 / total_items as f32
                * visible_items as f32)
                .round()
                .max(1.0) as u16;
            let scrollbar_position = (panel.scroll as f32 / total_items as f32
                * visible_items as f32)
                .round()
                .max(0.0) as u16;

            for i in 0..rows_available {
                if i >= scrollbar_position && i < scrollbar_position + scrollbar_height {
                    let _ = queue!(
                        &self.stdout,
                        cursor::MoveTo(self.columns - 1, HEADER_ROWS + i),
                        Print("|".dark_blue())
                    );
                }
            }
        }
    }

    fn draw_files_list_two_panes(
        &mut self,
        left: &PanelState,
        right: &PanelState,
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

            let left_index = left.scroll + row as usize;
            if left_index < left.entries.len() {
                let left_name = self.styled_name_for_index(left, left_index);
                let left_trunc = self.truncate_styled(left_name, pane_width);
                let _ = queue!(
                    &self.stdout,
                    cursor::MoveTo(0, screen_row),
                    Print(left_trunc)
                );
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

            let right_index = right.scroll + row as usize;
            if right_index < right.entries.len() {
                let right_name = self.styled_name_for_index(right, right_index);
                let right_trunc = self.truncate_styled(right_name, pane_width.saturating_sub(1));
                let _ = queue!(
                    &self.stdout,
                    cursor::MoveTo(right_col, screen_row),
                    Print(right_trunc)
                );
            }
        }
    }

    fn styled_name_for_index(&self, panel: &PanelState, index: usize) -> StyledContent<String> {
        let entry = &panel.entries[index];
        let name = self.style_entry_name(entry);

        let name_final = if index == panel.cursor {
            name.negative()
        } else {
            name
        };

        if panel.is_multi_selected(index) {
            name_final.on(Color::Blue)
        } else {
            name_final
        }
    }

    fn style_entry_name(&self, entry: &FileEntry) -> StyledContent<String> {
        if entry.is_dir() {
            entry.name.clone().yellow()
        } else {
            entry.name.clone().dark_yellow()
        }
    }

    fn draw_quick_view(&mut self, quick_view: &QuickViewMode) {
        let rows_available = self.rows - HEADER_ROWS - FOOTER_ROWS;
        match quick_view {
            QuickViewMode::Text {
                lines,
                start,
                length: _,
            } => {
                self.draw_text_content(lines, *start, rows_available);
            }
            QuickViewMode::Image(_data, bytes) => {
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
            }
            QuickViewMode::Directory { lines } => {
                self.draw_text_content(lines, 0, rows_available);
            }
            QuickViewMode::NotSupported => {
                self.draw_text_content(&["".to_string()], 0, rows_available);
            }
        }
    }

    fn draw_text_content(&self, content: &[String], start: usize, rows_available: u16) {
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

    fn draw_footer(&self, panel: &PanelState) {
        self.draw_full_line(self.rows - FOOTER_ROWS);

        let indicator_row = if panel.mode == PanelMode::Filter {
            format!("Filter: {}", panel.filter_string).with(Color::Green)
        } else if matches!(panel.mode, PanelMode::QuickView(_)) {
            format!("File {} / {}", panel.cursor + 1, panel.entries.len()).with(Color::Green)
        } else if panel.multi_selected_count() > 0 {
            format!("Selected: {}", panel.multi_selected_count()).with(Color::Yellow)
        } else {
            let total_dirs = panel.entries.iter().filter(|e| e.is_dir()).count();
            let total_files = panel
                .entries
                .len()
                .saturating_sub(total_dirs)
                .saturating_sub(1);
            format!("{} dirs, {} files", total_dirs, total_files).with(Color::Green)
        };

        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, self.rows - FOOTER_ROWS + 1),
            Print(indicator_row),
            terminal::Clear(ClearType::UntilNewLine),
        );
    }

    fn draw_footer_actions(&self, mode: &PanelMode) {
        let mut actions = self.get_footer_actions_by_mode(mode);

        while self.actions_width(&actions, 0) > self.columns as usize && actions.len() > 1 {
            actions.pop();
        }

        if actions.is_empty() {
            return;
        }

        actions.sort_by(|(a, pos_a), (b, pos_b)| match (pos_a, pos_b) {
            (FooterActionsPosition::Start, FooterActionsPosition::Start) => a.cmp(b),
            (FooterActionsPosition::Start, _) => std::cmp::Ordering::Less,
            (_, FooterActionsPosition::Start) => std::cmp::Ordering::Greater,
            (FooterActionsPosition::End, FooterActionsPosition::End) => a.cmp(b),
            (FooterActionsPosition::End, _) => std::cmp::Ordering::Greater,
            (_, FooterActionsPosition::End) => std::cmp::Ordering::Less,
            (FooterActionsPosition::Nondetermined, FooterActionsPosition::Nondetermined) => {
                a.cmp(b)
            }
        });

        let available = self.columns as usize;
        let content_width: usize = actions.iter().map(|(a, _)| a.len()).sum();

        let mut rendered = String::new();
        if actions.len() == 1 {
            let pad = available.saturating_sub(content_width) / 2;
            rendered.push_str(&" ".repeat(pad));
            rendered.push_str(&actions[0].0);
        } else {
            let gaps = actions.len() - 1;
            let extra_space = available.saturating_sub(content_width);
            let base_spacing = extra_space / gaps;
            let remainder = extra_space % gaps;

            for (idx, (action, _)) in actions.iter().enumerate() {
                rendered.push_str(action);
                if idx + 1 < actions.len() {
                    let bonus = if idx < remainder { 1 } else { 0 };
                    let spacing = base_spacing + bonus;
                    rendered.push_str(&" ".repeat(spacing.max(1)));
                }
            }
        }

        let actions_row = rendered
            .chars()
            .take(self.columns as usize)
            .collect::<String>()
            .with(Color::Green);

        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, self.rows - FOOTER_ROWS + 2),
            Print(actions_row),
            terminal::Clear(ClearType::UntilNewLine),
        );
    }

    fn get_footer_actions_by_mode(&self, mode: &PanelMode) -> Vec<(String, FooterActionsPosition)> {
        match mode {
            PanelMode::Normal => vec![
                ("[f1]help".into(), FooterActionsPosition::Start),
                ("[f3]view".into(), FooterActionsPosition::Nondetermined),
                ("[f4]edit".into(), FooterActionsPosition::Nondetermined),
                ("[f6]find".into(), FooterActionsPosition::Nondetermined),
                ("[f7]ripgrep".into(), FooterActionsPosition::Nondetermined),
                (
                    "[ctrl+w]delete".into(),
                    FooterActionsPosition::Nondetermined,
                ),
                ("[f10]quit".into(), FooterActionsPosition::End),
            ],
            PanelMode::Filter => vec![
                ("[f1]help".into(), FooterActionsPosition::Start),
                ("[esc]clear".into(), FooterActionsPosition::End),
                ("[f3]view".into(), FooterActionsPosition::Nondetermined),
                ("[f10]quit".into(), FooterActionsPosition::End),
            ],
            PanelMode::QuickView(_) => vec![
                ("[f1]help".into(), FooterActionsPosition::Start),
                (
                    "[up/down]scroll".into(),
                    FooterActionsPosition::Nondetermined,
                ),
                (
                    "[pgup/pgdn]page".into(),
                    FooterActionsPosition::Nondetermined,
                ),
                (
                    "[left/right]prev/next".into(),
                    FooterActionsPosition::Nondetermined,
                ),
                (
                    "[f3]close view".into(),
                    FooterActionsPosition::Nondetermined,
                ),
            ],
        }
    }

    fn actions_width(&self, actions: &[(String, FooterActionsPosition)], sep_len: usize) -> usize {
        if actions.is_empty() {
            return 0;
        }
        actions.iter().map(|(a, _)| a.len()).sum::<usize>() + sep_len * (actions.len() - 1)
    }

    fn draw_help_overlay(&mut self) {
        let mut entries = self.help_entries.clone();
        entries.sort_by_key(|(k, _)| k.clone());

        let title = "Keyboard shortcuts";
        let col_gap = HELP_OVERLAY_COL_GAP;
        const OVERLAY_PADDING: usize = 4;
        let max_key = entries.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        let max_desc = entries.iter().map(|(_, d)| d.len()).max().unwrap_or(0);
        let total_width =
            (max_key + col_gap + max_desc + OVERLAY_PADDING).min(self.columns as usize);
        let start_col = ((self.columns as usize).saturating_sub(total_width)) / 2;

        let start_row = HELP_OVERLAY_START_ROW;
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, start_row),
            terminal::Clear(ClearType::FromCursorDown)
        );

        // Title
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(start_col as u16, start_row),
            Print(
                title
                    .with(Color::Cyan)
                    .attribute(crossterm::style::Attribute::Bold)
            )
        );

        // Separator
        let sep = "─".repeat(total_width.min(self.columns as usize));
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(start_col as u16, start_row + 1),
            Print(sep.with(Color::Blue))
        );

        // Entries
        let mut row = start_row + 2;
        for (key, desc) in entries {
            if row >= self.rows.saturating_sub(1) {
                break;
            }
            let padded_key = format!("{:width$}", key, width = max_key);
            let _ = queue!(
                &self.stdout,
                cursor::MoveTo(start_col as u16, row),
                Print(padded_key.with(Color::Yellow)),
                cursor::MoveTo((start_col + max_key + col_gap) as u16, row),
                Print(desc.with(Color::Green))
            );
            row += 1;
        }
    }

    fn draw_full_line(&self, row: u16) {
        let text = (0..self.columns)
            .map(|_| "─")
            .collect::<String>()
            .with(Color::Blue);
        let _ = queue!(&self.stdout, cursor::MoveTo(0, row), Print(text));
    }

    fn truncate_styled(&self, item: StyledContent<String>, width: u16) -> StyledContent<String> {
        let text = item.content().clone();
        if text.len() as u16 > width && width > 1 {
            let mut truncated = text.chars().take((width - 1) as usize).collect::<String>();
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

    /// Resets the terminal to its original state.
    pub fn reset_terminal(&mut self) {
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            cursor::Show,
            cursor::EnableBlinking,
            terminal::Clear(ClearType::All)
        );
        let _ = terminal::disable_raw_mode();
        let _ = self.stdout.flush();
    }

    /// Gets the number of visible rows for file listing.
    pub fn visible_rows(&self) -> u16 {
        self.rows - HEADER_ROWS - FOOTER_ROWS
    }
}

impl Drop for TerminalRenderer {
    fn drop(&mut self) {
        self.reset_terminal();
    }
}
