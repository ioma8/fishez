use std::sync::mpsc::Sender;

use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent},
    queue,
    style::{Print, StyledContent, Stylize},
    terminal::{self, ClearType},
};

use crate::{
    files_view::{FilesView, FOOTER_ROWS},
    terminal_ui::{FeatureTrait, Message, TerminalUI},
};

pub struct MultiSelectFeature {
    selected_indexes: Vec<usize>,
}

// TODO: tato featura neni jeste funkcni
impl Default for MultiSelectFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiSelectFeature {
    pub fn new() -> Self {
        Self {
            selected_indexes: Vec::new(),
        }
    }
}

impl FeatureTrait for MultiSelectFeature {
    fn captured_key_event(
        &mut self,
        event: KeyEvent,
        files_view: &mut FilesView,
        _: &Sender<Message>,
    ) -> bool {
        if let KeyCode::Char(' ') = event.code {
            let hovered_index = files_view.selected;
            if self.selected_indexes.contains(&hovered_index) {
                self.selected_indexes.retain(|&x| x != hovered_index);
            } else {
                self.selected_indexes.push(hovered_index);
            }
            return true;
        }
        false
    }

    fn drawn_footer(&self, _: &FilesView, terminal_ui: &TerminalUI) -> bool {
        let _ = queue!(
            &terminal_ui.stdout,
            cursor::MoveTo(0, terminal_ui.rows - FOOTER_ROWS + 1),
            Print(format!("Selected: {:?}", self.selected_indexes.len()).yellow()),
            terminal::Clear(ClearType::UntilNewLine),
        );
        true
    }

    fn map_item(&self, item: StyledContent<String>, index: usize) -> StyledContent<String> {
        if self.selected_indexes.contains(&index) {
            item.on(crossterm::style::Color::Blue)
        } else {
            item
        }
    }
}
