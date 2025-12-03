//! File operations use cases - delete, copy, etc.

use crate::application::ports::{ClipboardPort, FileSystemPort};
use crate::application::state::PanelState;
use crate::application::use_cases::navigate;
use std::path::Path;

/// Deletes the selected file or directory (moves to trash).
#[allow(dead_code)]
pub fn delete_selected(
    fs: &dyn FileSystemPort,
    panel: &mut PanelState,
    paths: &[&Path],
) -> Result<(), String> {
    for path in paths {
        fs.delete(path)?;
    }
    panel.clear_multi_selection();
    navigate::refresh_entries(fs, panel);
    Ok(())
}

/// Copies the selected file path to clipboard.
pub fn copy_to_clipboard(
    clipboard: &mut dyn ClipboardPort,
    panel: &mut PanelState,
    absolute_path: bool,
) -> Result<(), String> {
    if panel.entries.is_empty() || panel.cursor >= panel.entries.len() {
        return Err("No file selected".to_string());
    }

    let entry = &panel.entries[panel.cursor];
    let text = if absolute_path {
        entry.path.to_string_lossy().to_string()
    } else {
        entry.name.clone()
    };

    clipboard.copy(&text)?;

    let msg = if absolute_path {
        format!("Copied absolute path to clipboard: {}", text)
    } else {
        format!("Copied name to clipboard: {}", text)
    };
    panel.set_notification(msg);

    Ok(())
}
