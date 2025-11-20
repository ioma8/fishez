use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent},
    queue,
    style::{Print, Stylize},
    terminal::{self, ClearType},
};

use crate::{
    files_view::{FilesView, FOOTER_ROWS},
    terminal_ui::{FeatureTrait, Message, TerminalUI},
};
use std::{process::Command, sync::mpsc::Sender};

pub struct RipGrepFeature {
    filter: Option<String>,
}

impl Default for RipGrepFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl RipGrepFeature {
    pub fn new() -> Self {
        Self { filter: None }
    }

    pub fn find(&self) -> Vec<String> {
        if let Some(filter) = &self.filter {
            let output = Command::new("rg")
                .arg(filter)
                .output()
                .expect("Failed to execute rg command");

            if output.status.success() {
                let result = String::from_utf8_lossy(&output.stdout);
                return result
                    .lines()
                    .map(|s| s.to_string())
                    .map(|s| {
                        let mut parts = s.split(':');
                        let path = parts.next().unwrap();
                        //let line = parts.next().unwrap();
                        //let content = parts.next().unwrap();
                        path.to_string()
                    })
                    .collect();
            }
        }
        Vec::new()
    }
}

impl FeatureTrait for RipGrepFeature {
    fn captured_key_event(
        &mut self,
        event: KeyEvent,
        files_view: &mut FilesView,
        _: &Sender<Message>,
    ) -> bool {
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
        } else if event.code == KeyCode::F(7) {
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
                Print(format!("RipGrep: {} [enter/esc]", filter).magenta().bold()),
            );
            return true;
        }
        false
    }

    fn footer_help(&self) -> Vec<String> {
        vec!["[f7]ripgrep".into()]
    }

    fn overlay_help(&self) -> Vec<(String, String)> {
        vec![("F7".into(), "RipGrep search".into())]
    }
}

fn _ripgrep_search(dir: &str, query: &str, results: &mut Vec<String>) {
    let output = Command::new("cmd")
        .args(["/C", "findstr", "/s", "/i", "/p", query, "*"])
        .current_dir(dir)
        .output()
        .expect("Failed to execute findstr");

    if output.status.success() {
        let result_str = String::from_utf8_lossy(&output.stdout);
        let paths = result_str
            .lines()
            .filter_map(|line| line.split(':').next())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .map(String::from);
        results.extend(paths);
    }

    // TODO: implementace pro linux a macos: nejdřív zkusí najít command rg
    // (pomcí --version při startu programu), pokud není tak použije grep
}
