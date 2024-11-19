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
        if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", "code", path])
                .spawn()
                .unwrap();
        } else if cfg!(target_os = "macos") {
            Command::new("open")
                .arg("-a")
                .arg("Visual Studio Code")
                .arg(path)
                .spawn()
                .unwrap();
        } else {
            Command::new("code").arg(path).spawn().unwrap();
        }
    }
}

impl FeatureTrait for VsCodeFeature {
    fn captured_key_event(
        &mut self,
        event: KeyEvent,
        files_view: &mut FilesView,
        sender: &Sender<Message>,
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

    fn modify_footer_actions(&self, actions: &mut Vec<&str>) {
        actions.push("[f4]edit");
    }
}
