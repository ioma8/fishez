use std::sync::mpsc::Sender;

use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent},
    queue,
    style::{Print, StyledContent, Stylize},
    terminal::{self, ClearType},
};

use crate::{
    files_view::{FOOTER_ROWS, FilesView, FilesViewMode},
    terminal_ui::{FeatureTrait, Message, TerminalUI},
};

pub struct MultiSelectFeature {}

// TODO: tato featura neni jeste funkcni
impl Default for MultiSelectFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiSelectFeature {
    pub fn new() -> Self {
        Self {}
    }
}

impl FeatureTrait for MultiSelectFeature {
    fn captured_key_event(
        &mut self,
        event: KeyEvent,
        files_view: &mut FilesView,
        _: &Sender<Message>,
    ) -> bool {
        // Ignore quick view; selection only makes sense in list modes.
        if matches!(files_view.mode, FilesViewMode::QuickView(_)) {
            return false;
        }

        if let KeyCode::Char(' ') = event.code {
            files_view.toggle_multi_selection(files_view.selected);
            return true;
        }

        // Allow Esc to clear selection without leaving the view.
        if event.code == KeyCode::Esc && files_view.multi_selected_count() > 0 {
            files_view.clear_multi_selection();
            return true;
        }

        false
    }

    fn drawn_footer(&self, files_view: &FilesView, terminal_ui: &TerminalUI) -> bool {
        // We can't mutate the selection here without interior mutability.
        // Just show the footer when there's something selected.
        let count = files_view.multi_selected_count();
        if count == 0 {
            return false;
        }

        let _ = queue!(
            &terminal_ui.stdout,
            cursor::MoveTo(0, terminal_ui.rows - FOOTER_ROWS + 1),
            Print(format!("Selected: {}", count).yellow()),
            terminal::Clear(ClearType::UntilNewLine),
        );
        true
    }

    fn map_item(
        &self,
        item: StyledContent<String>,
        index: usize,
        files_view: &FilesView,
    ) -> StyledContent<String> {
        if files_view.is_multi_selected(index) {
            item.on(crossterm::style::Color::Blue)
        } else {
            item
        }
    }

    fn footer_help(&self) -> Vec<String> {
        vec![]
    }

    fn overlay_help(&self) -> Vec<(String, String)> {
        vec![("Space".into(), "Toggle selection for batch actions".into())]
    }
}
