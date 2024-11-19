use std::{path::MAIN_SEPARATOR, process::Command};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{files_view::FilesView, terminal_ui::FeatureTrait};

pub struct OpenFeature {}

impl Default for OpenFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenFeature {
    pub fn new() -> Self {
        Self {}
    }

    fn open(&self, path: &str) {
        if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", "start", "", path])
                .spawn()
                .unwrap();
        } else if cfg!(target_os = "macos") {
            Command::new("open").arg(path).spawn().unwrap();
        } else {
            Command::new("xdg-open").arg(path).spawn().unwrap();
        }
    }

    fn is_file(&self, path: &str) -> bool {
        std::fs::metadata(path)
            .map(|m| m.is_file())
            .unwrap_or(false)
    }
}

impl FeatureTrait for OpenFeature {
    fn captured_key_event(&mut self, event: KeyEvent, files_view: &mut FilesView) -> bool {
        let selected_file = &files_view.files[files_view.selected];
        if selected_file == ".." {
            return false;
        }

        let file_path = format!("{}{}{}", files_view.pwd, MAIN_SEPARATOR, selected_file);
        let is_file = self.is_file(&file_path);

        if !is_file {
            return false;
        }

        if event.code == KeyCode::Enter {
            self.open(&file_path);
            return true;
        }

        false
    }
}
