use std::sync::mpsc::Sender;

use crossterm::{
    cursor,
    event::{self, KeyCode, KeyEvent},
    queue,
    style::{Print, Stylize},
    terminal::{self, ClearType},
};

use crate::{
    files_view::{FilesView, FOOTER_ROWS, HEADER_ROWS},
    terminal_ui::{FeatureTrait, Message, TerminalUI},
};

pub struct FavouritesFeature {
    items: Vec<String>,
    selected_index: usize,
    active: bool,
}

impl Default for FavouritesFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl FavouritesFeature {
    pub fn new() -> Self {
        let items = std::fs::read_to_string("favorites.txt")
            .unwrap_or_else(|_| String::new())
            .lines()
            .map(|line| line.to_string())
            .collect();
        Self {
            items,
            selected_index: 0,
            active: false,
        }
    }
}

impl FeatureTrait for FavouritesFeature {
    fn captured_key_event(
        &mut self,
        event: KeyEvent,
        files_view: &mut FilesView,
        _: &Sender<Message>,
    ) -> bool {
        if !self.active {
            if event.code == KeyCode::Char('d')
                && event.modifiers.contains(event::KeyModifiers::CONTROL)
            {
                if event.modifiers.contains(event::KeyModifiers::SHIFT) {
                    // TODO: not workimg on macos
                    if let Some(selected_file) = files_view.get_selected_file_abs() {
                        self.items.push(selected_file);
                        self.items.dedup();
                        let _ = std::fs::write("favorites.txt", self.items.join("\n"));
                    }
                }
                self.active = true;
                return true;
            } else {
                return false;
            }
        }

        match event.code {
            KeyCode::Up => self.selected_index = self.selected_index.saturating_sub(1),
            KeyCode::Down => {
                self.selected_index = self
                    .selected_index
                    .saturating_add(1)
                    .min(self.items.len() - 1)
            }
            KeyCode::Enter => {
                if let Some(item) = self.items.get(self.selected_index) {
                    files_view.pwd = item.clone();
                    files_view.update();
                    self.active = false;
                }
            }
            KeyCode::Esc => self.active = false,
            _ => {}
        }

        true
    }

    fn drawn_content(&self, _: &FilesView, terminal_ui: &TerminalUI) -> bool {
        if !self.active {
            return false;
        }

        let rows_available = terminal_ui.rows - HEADER_ROWS - FOOTER_ROWS;
        // TODO: add scroll of items if they dont fit on screen

        let _ = queue!(&terminal_ui.stdout, cursor::MoveTo(0, HEADER_ROWS));

        for (i, item) in self.items.iter().enumerate() {
            let name = if i == self.selected_index {
                item.clone().dark_magenta().negative()
            } else {
                item.clone().dark_magenta()
            };

            let _ = queue!(
                &terminal_ui.stdout,
                Print(name),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }

        let rows_to_clear: i16 = rows_available as i16 - self.items.len() as i16;

        if rows_to_clear > 0 {
            for _ in 0..rows_to_clear {
                let _ = queue!(
                    &terminal_ui.stdout,
                    terminal::Clear(ClearType::UntilNewLine),
                    cursor::MoveToNextLine(1)
                );
            }
        }

        true
    }

    fn drawn_header(&self, _: &FilesView, terminal_ui: &TerminalUI) -> bool {
        if !self.active {
            return false;
        }
        let _ = queue!(
            &terminal_ui.stdout,
            cursor::MoveTo(0, 0),
            Print("Favourites"),
            terminal::Clear(ClearType::UntilNewLine)
        );
        true
    }

    fn footer_help(&self) -> Vec<String> {
        vec!["[ctrl+d]favourites".into()]
    }

    fn overlay_help(&self) -> Vec<(String, String)> {
        vec![("Ctrl+D".into(), "Favorites list / add current dir".into())]
    }
}
