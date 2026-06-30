//! Open adapters for launching files with system applications.

use std::path::Path;
use std::process::Command;

/// Opens files with the OS default application.
#[derive(Default)]
pub struct SystemOpenAdapter;

impl SystemOpenAdapter {
    pub fn open(&self, path: &Path) {
        let path_str = path.to_string_lossy();
        if cfg!(target_os = "windows") {
            let _ = Command::new("cmd")
                .args(["/C", "start", "", &path_str])
                .spawn();
        } else if cfg!(target_os = "macos") {
            let _ = Command::new("open").arg(&*path_str).spawn();
        } else {
            let _ = Command::new("xdg-open").arg(&*path_str).spawn();
        }
    }
}

/// Opens files in VS Code.
#[derive(Default)]
pub struct VsCodeAdapter;

impl VsCodeAdapter {
    pub fn open(&self, path: &Path) {
        let path_str = path.to_string_lossy();
        if cfg!(target_os = "windows") {
            let _ = Command::new("cmd")
                .args(["/C", "start", "code", &path_str])
                .spawn();
        } else if cfg!(target_os = "macos") {
            let _ = Command::new("open")
                .arg("-a")
                .arg("Visual Studio Code")
                .arg(&*path_str)
                .spawn();
        } else {
            let _ = Command::new("code").arg(&*path_str).spawn();
        }
    }
}
