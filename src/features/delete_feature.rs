use crossterm::{cursor, event::{self, KeyCode, KeyEvent}, queue, style::{Print, Stylize}, terminal::{self, ClearType}};

use crate::{files_view::{FilesView, FOOTER_ROWS, HEADER_ROWS}, terminal_ui::{FeatureTrait, TerminalUI}};


pub struct FavouritesFeature {
    items: Vec<String>,
    selected_index: usize,
}

impl FavouritesFeature {
    pub fn new() -> Self {
        let items = std::fs::read_to_string("favorites.txt")
            .unwrap_or_else(|_| String::new())
            .lines()
            .map(|line| line.to_string())
            .collect();
        Self {
            items: items,
            selected_index: 0,
        }
    }
}

impl FeatureTrait for FavouritesFeature {
    fn get_id(&self) -> &'static str {
        "favourites"
    }

    fn general_shortcuts(&mut self, event: KeyEvent, files_view: &mut FilesView) -> bool {
        if event.code == KeyCode::Char('d')
            && event.modifiers.contains(event::KeyModifiers::CONTROL)
        {
            files_view.feature_active = Some(self.get_id().to_string());
            return true;
        }
        return false;
    }

    fn view_shortcuts(&mut self, event: KeyEvent, files_view: &mut FilesView) -> bool {
        match event.code {
            KeyCode::Up => self.selected_index = self.selected_index.saturating_sub(1),
            KeyCode::Down => self.selected_index = self.selected_index.saturating_add(1),
            KeyCode::Enter => {
                if let Some(item) = self.items.get(self.selected_index) {
                    files_view.pwd = item.clone();
                    files_view.update();
                    files_view.feature_active = None;
                }
            }
            KeyCode::Esc => files_view.feature_active = None,
            _ => return false
        }
        return true;
    }

    fn draw_content(&self, files_view: &FilesView, terminal_ui: &TerminalUI) -> bool {
        let rows_available = terminal_ui.rows - HEADER_ROWS - FOOTER_ROWS;
        // TODO: add scroll of items if they dont fit on screen

        let _ = queue!(&terminal_ui.stdout, cursor::MoveTo(0, HEADER_ROWS));

        for (i, item) in self.items.iter().enumerate() {
            let name = if (i == self.selected_index) {
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

        return true;
    }
    
    fn draw_header(&self, files_view: &FilesView, terminal_ui: &TerminalUI) -> bool {
        let _ = queue!(&terminal_ui.stdout, cursor::MoveTo(0, 0), Print("Favourites"));
        true
    }
}