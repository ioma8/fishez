//! Open adapter - implements OpenPort for opening files with system applications.

use crate::application::ports::OpenPort;
use std::path::Path;
use std::process::Command;

/// System open adapter.
pub struct SystemOpenAdapter;

impl Default for SystemOpenAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemOpenAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl OpenPort for SystemOpenAdapter {
    fn open(&self, path: &Path) {
        let path_str = path.to_string_lossy();
        // We intentionally don't wait for the process to complete
        // as we want the application to launch independently
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

/// VS Code open adapter.
pub struct VsCodeAdapter;

impl Default for VsCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VsCodeAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl OpenPort for VsCodeAdapter {
    fn open(&self, path: &Path) {
        let path_str = path.to_string_lossy();
        // We intentionally don't wait for VSCode process to complete
        // as we want it to launch and continue independently
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
