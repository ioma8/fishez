use std::{path::MAIN_SEPARATOR, process::Command, sync::mpsc::Sender};

use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    files_view::FilesView,
    terminal_ui::{FeatureTrait, Message},
};

pub struct VsCodeFeature {}

impl Default for VsCodeFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl VsCodeFeature {
    pub fn new() -> Self {
        Self {}
    }
    fn open(&self, path: &str) {
        // We intentionally don't wait for VSCode process to complete
        // as we want it to launch and continue independently
        if cfg!(target_os = "windows") {
            let _ = Command::new("cmd")
                .args(["/C", "start", "code", path])
                .spawn();
        } else if cfg!(target_os = "macos") {
            let _ = Command::new("open")
                .arg("-a")
                .arg("Visual Studio Code")
                .arg(path)
                .spawn();
        } else {
            let _ = Command::new("code").arg(path).spawn();
        }
    }
}

impl FeatureTrait for VsCodeFeature {
    fn captured_key_event(
        &mut self,
        event: KeyEvent,
        files_view: &mut FilesView,
        _: &Sender<Message>,
    ) -> bool {
        let selected_file = &files_view.files[files_view.selected];
        if selected_file == ".." {
            return false;
        }

        let file_path = format!("{}{}{}", files_view.pwd, MAIN_SEPARATOR, selected_file);

        if event.code == KeyCode::F(4) {
            self.open(&file_path);
            return true;
        }

        false
    }

    fn footer_help(&self) -> Vec<String> {
        vec!["[f4]edit".into()]
    }

    fn overlay_help(&self) -> Vec<(String, String)> {
        vec![("F4".into(), "Open in VS Code".into())]
    }
}
