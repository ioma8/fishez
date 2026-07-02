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

/// Opens files in $EDITOR, falling back to VS Code.
#[derive(Default)]
pub struct VsCodeAdapter;

impl VsCodeAdapter {
    /// The `$EDITOR` command split into program + args, if configured.
    pub fn terminal_editor(&self) -> Option<Vec<String>> {
        editor_command_from(std::env::var("EDITOR").ok())
    }

    /// Runs a terminal editor on the file and blocks until it exits.
    /// The caller must suspend the TUI around this (see `TerminalRenderer::suspend`).
    pub fn open_in_terminal(&self, parts: &[String], path: &Path) {
        let _ = Command::new(&parts[0]).args(&parts[1..]).arg(path).status();
    }

    /// GUI fallback: opens the file in VS Code without blocking.
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

fn editor_command_from(value: Option<String>) -> Option<Vec<String>> {
    let value = value?;
    let parts: Vec<String> = value.split_whitespace().map(String::from).collect();
    (!parts.is_empty()).then_some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_command_from_handles_missing_and_blank() {
        assert_eq!(editor_command_from(None), None);
        assert_eq!(editor_command_from(Some("   ".to_string())), None);
    }

    #[test]
    fn editor_command_from_splits_command_and_args() {
        assert_eq!(
            editor_command_from(Some("vim".to_string())),
            Some(vec!["vim".to_string()])
        );
        assert_eq!(
            editor_command_from(Some("code -w".to_string())),
            Some(vec!["code".to_string(), "-w".to_string()])
        );
    }
}
