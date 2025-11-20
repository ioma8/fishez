use std::{path::MAIN_SEPARATOR, process::Command, sync::mpsc::Sender};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    files_view::FilesView,
    terminal_ui::{FeatureTrait, Message},
};

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
        // We intentionally don't wait for the process to complete
        // as we want the application to launch independently
        if cfg!(target_os = "windows") {
            let _ = Command::new("cmd").args(["/C", "start", "", path]).spawn();
        } else if cfg!(target_os = "macos") {
            let _ = Command::new("open").arg(path).spawn();
        } else {
            let _ = Command::new("xdg-open").arg(path).spawn();
        }
    }

    fn is_file(&self, path: &str) -> bool {
        std::fs::metadata(path)
            .map(|m| m.is_file())
            .unwrap_or(false)
    }
}

impl FeatureTrait for OpenFeature {
    fn captured_key_event(
        &mut self,
        event: KeyEvent,
        files_view: &mut FilesView,
        _: &Sender<Message>,
    ) -> bool {
        if files_view.files.is_empty() || files_view.selected >= files_view.files.len() {
            return false;
        }

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
