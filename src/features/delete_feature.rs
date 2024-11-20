use std::sync::mpsc::Sender;

use crossterm::{
    cursor,
    event::{self, KeyCode, KeyEvent},
    queue,
    style::{Print, Stylize},
    terminal::{self, ClearType},
};

use crate::{
    files_view::{FilesView, FOOTER_ROWS},
    terminal_ui::{FeatureTrait, Message, TerminalUI},
};

pub struct DeleteFeature {
    path: Option<String>,
}

impl Default for DeleteFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl DeleteFeature {
    pub fn new() -> Self {
        Self { path: None }
    }

    fn delete_file_or_dir(&self, path: &str) {
        if let Err(e) = trash::delete(path) {
            eprintln!("Error moving file or directory to trash: {}", e);
        }
    }
}

impl FeatureTrait for DeleteFeature {
    fn captured_key_event(
        &mut self,
        event: KeyEvent,
        files_view: &mut FilesView,
        _: &Sender<Message>,
    ) -> bool {
        if let Some(path) = &self.path {
            match event.code {
                KeyCode::Char('y') => {
                    self.delete_file_or_dir(path);
                    self.path = None;
                    files_view.update();
                }
                KeyCode::Char('n') => {
                    self.path = None;
                }
                KeyCode::Esc => {
                    self.path = None;
                }
                _ => return false,
            }
            return true;
        } else if event.code == KeyCode::Char('w')
            && event.modifiers.contains(event::KeyModifiers::CONTROL)
        {
            if let Some(selected_file) = files_view.get_selected_file_abs() {
                self.path = Some(selected_file.clone());
                return true;
            }
        }
        false
    }

    fn drawn_footer(&self, _: &FilesView, terminal_ui: &TerminalUI) -> bool {
        if let Some(path) = &self.path {
            let _ = queue!(
                &terminal_ui.stdout,
                cursor::MoveTo(0, terminal_ui.rows - FOOTER_ROWS + 1),
                terminal::Clear(ClearType::UntilNewLine),
                Print(format!("Delete: {}? [y/n]", path).red().bold()),
            );
            return true;
        }
        false
    }

    fn modify_footer_actions(&self, actions: &mut Vec<&str>) {
        actions.push("[ctrl+w]delete");
    }
}
