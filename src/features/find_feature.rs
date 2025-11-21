use crossterm::{
    cursor,
    event::{KeyCode, KeyEvent},
    queue,
    style::{Print, Stylize},
    terminal::{self, ClearType},
};

use crate::{
    files_view::{FOOTER_ROWS, FilesView},
    terminal_ui::{FeatureTrait, Message, TerminalUI},
};
use std::{
    path::{MAIN_SEPARATOR, Path},
    process::Command,
    sync::mpsc::Sender,
    thread,
};

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

    pub fn find(&self, current_dir: String) -> Vec<String> {
        if let Some(filter) = &self.filter {
            if cfg!(target_os = "windows") {
                return self.find_windows(filter, &current_dir);
            } else {
                return self.find_unix(filter, &current_dir);
            }
        }
        Vec::new()
    }

    fn find_unix(&self, filter: &str, dir: &str) -> Vec<String> {
        let output = Command::new("fd").arg(filter).current_dir(dir).output();
        if let Ok(output) = output && output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout);
            return result
                .lines()
                .map(|s| s.to_string())
                .map(|rel| {
                    let abs = Path::new(dir).join(&rel);
                    if abs.is_dir() {
                        format!("{}{}", rel, MAIN_SEPARATOR)
                    } else {
                        rel
                    }
                })
                .collect();
        }
        Vec::new()
    }

    fn find_windows(&self, filter: &str, dir: &str) -> Vec<String> {
        let output = Command::new("cmd")
            .args(["/C", "dir", "/s", "/b", &format!("*{}*", filter)])
            .current_dir(dir)
            .output();

        if let Ok(output) = output && output.status.success() {
            let mut results = Vec::new();
            let result_str = String::from_utf8_lossy(&output.stdout);
            for line in result_str.lines() {
                let abs_path = Path::new(line);
                let path_relative = line.trim_start_matches(dir);
                if abs_path.is_dir() {
                    results.push(format!("{}{}", path_relative, MAIN_SEPARATOR));
                } else {
                    results.push(path_relative.to_string());
                }
            }
            return results;
        }
        Vec::new()
    }
}

impl FeatureTrait for FindFeature {
    fn captured_key_event(
        &mut self,
        event: KeyEvent,
        files_view: &mut FilesView,
        sender: &Sender<Message>,
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
                    let sender = sender.clone();
                    let pwd = files_view.pwd.clone();
                    let filter = self.filter.clone();
                    files_view.set_notification("Searching...".to_string());
                    thread::spawn(move || {
                        let find_feature = FindFeature { filter };
                        let files = find_feature.find(pwd);
                        sender.send(Message::DrawFiles(files)).unwrap();
                    });
                }
                KeyCode::Esc => {
                    self.filter = None;
                    files_view.update();
                }
                _ => return false,
            }
            return true;
        } else if event.code == KeyCode::F(6) {
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

    fn footer_help(&self) -> Vec<String> {
        vec!["[f6]find".into()]
    }

    fn overlay_help(&self) -> Vec<(String, String)> {
        vec![("F6".into(), "Find (fd)".into())]
    }
}
