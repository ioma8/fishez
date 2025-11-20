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
    paths: Option<Vec<String>>,
}

impl Default for DeleteFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl DeleteFeature {
    pub fn new() -> Self {
        Self { paths: None }
    }

    fn delete_file_or_dir(&self, path: &str) {
        if let Err(e) = trash::delete(path) {
            eprintln!("Error moving file or directory to trash: {}", e);
        }
    }

    fn format_prompt(&self, paths: &[String]) -> String {
        match paths {
            [] => "Delete: nothing selected".to_string(),
            [single] => format!("Delete: {}? [y/n]", single),
            many => {
                let first = many.first().unwrap();
                format!("Delete: {} (+{} more)? [y/n]", first, many.len() - 1)
            }
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
        if let Some(paths) = &self.paths {
            match event.code {
                KeyCode::Char('y') => {
                    for path in paths {
                        self.delete_file_or_dir(path);
                    }
                    self.paths = None;
                    files_view.clear_multi_selection();
                    files_view.update();
                }
                KeyCode::Char('n') => {
                    self.paths = None;
                }
                KeyCode::Esc => {
                    self.paths = None;
                }
                _ => return false,
            }
            return true;
        } else if event.code == KeyCode::Char('w')
            && event.modifiers.contains(event::KeyModifiers::CONTROL)
        {
            let mut targets = if files_view.multi_selected_count() > 0 {
                files_view.multi_selected_paths()
            } else if let Some(selected_file) = files_view.get_selected_file_abs() {
                vec![selected_file]
            } else {
                vec![]
            };

            targets.retain(|p| !p.is_empty());

            if !targets.is_empty() {
                self.paths = Some(targets.clone());
                return true;
            }
        }
        false
    }

    fn drawn_footer(&self, _: &FilesView, terminal_ui: &TerminalUI) -> bool {
        if let Some(paths) = &self.paths {
            let _ = queue!(
                &terminal_ui.stdout,
                cursor::MoveTo(0, terminal_ui.rows - FOOTER_ROWS + 1),
                terminal::Clear(ClearType::UntilNewLine),
                Print(self.format_prompt(paths).red().bold()),
            );
            return true;
        }
        false
    }

    fn footer_help(&self) -> Vec<String> {
        vec!["[ctrl+w]delete".into()]
    }

    fn overlay_help(&self) -> Vec<(String, String)> {
        vec![("Ctrl+W".into(), "Delete selected/current to trash".into())]
    }
}
