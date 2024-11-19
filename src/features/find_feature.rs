use crossterm::{cursor, event::{KeyCode, KeyEvent}, queue, style::{Print, Stylize}, terminal::{self, ClearType}};

use crate::{files_view::{FilesView, FOOTER_ROWS}, terminal_ui::{FeatureTrait, TerminalUI}};
use std::process::Command;


pub struct FindFeature {
    filter: Option<String>,
}

impl Default for FindFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl FindFeature {
    pub fn new() -> Self {
        Self { filter: None }
    }

    pub fn find(&self) -> Vec<String> {
        if let Some(filter) = &self.filter {
            let output = Command::new("fd")
                .arg(filter)
                .output()
                .expect("Failed to execute fd command");

            if output.status.success() {
                let result = String::from_utf8_lossy(&output.stdout);
                return result.lines().map(|s| s.to_string()).collect();
            }
        }
        Vec::new()
    }
}

impl FeatureTrait for FindFeature {
    fn captured_key_event(&mut self, event: KeyEvent, files_view: &mut FilesView) -> bool {
        if let Some(filter) = &mut self.filter {
            match event.code {
                KeyCode::Char(c) => {
                    filter.push(c);
                }
                KeyCode::Backspace => {
                    filter.pop();
                }
                KeyCode::Enter => {
                    files_view.files = self.find();
                }
                KeyCode::Esc => {
                    self.filter = None;
                    files_view.update();
                }
                _ => return false,
            }
            return true;
        }
        else if event.code == KeyCode::F(6){
            self.filter = Some(String::new());
            return true;
        }
        false
    }

    fn drawn_footer(&self, _: &FilesView, terminal_ui: &TerminalUI) -> bool {
        if let Some(filter) = &self.filter {
            let _ = queue!(
                &terminal_ui.stdout,
                cursor::MoveTo(0, terminal_ui.rows - FOOTER_ROWS + 1),
                terminal::Clear(ClearType::UntilNewLine),
                Print(format!("Find: {} [enter/esc]", filter).magenta().bold()),
            );
            return true;
        }
        false
    }

    fn modify_footer_actions(&self, actions: &mut Vec<&str>) {
        actions.push("[f6]find");
    }
}