use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent},
    queue,
    style::{Print, StyledContent, Stylize},
    terminal::{self, ClearType},
};

use crate::{
    files_view::{FilesView, FOOTER_ROWS},
    terminal_ui::{FeatureTrait, TerminalUI},
};

// TODO: tato featura neni jeste funkcni

pub struct MultiSelectFeature {
    selected_indexes: Vec<usize>,
}

impl MultiSelectFeature {
    pub fn new() -> Self {
        Self {
            selected_indexes: Vec::new(),
        }
    }
}

impl FeatureTrait for MultiSelectFeature {
    fn captured_key_event(&mut self, event: KeyEvent, files_view: &mut FilesView) -> bool {
        match event.code {
            KeyCode::Char(' ') => {
                let hovered_index = files_view.selected;
                if self.selected_indexes.contains(&hovered_index)
                {
                    // TODO: remove item from selected_indexes not working properly rn
                    self.selected_indexes.remove(hovered_index);
                } else {
                    self.selected_indexes.push(hovered_index);
                }
                return true;
            }
            _ => {},
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
