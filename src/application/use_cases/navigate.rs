//! Navigation use cases - handles directory navigation and cursor movement.

use crate::application::ports::FileSystemPort;
use crate::application::state::{PanelMode, PanelState};
use crate::domain::{EntryKind, FileEntry};
use std::path::{MAIN_SEPARATOR, Path, PathBuf};

/// Refreshes the panel entries from the file system.
pub fn refresh_entries(fs: &dyn FileSystemPort, panel: &mut PanelState) {
    panel.multi_selected.clear();
    panel.entries.clear();

    // Add parent directory entry unless in filter mode
    if panel.mode == PanelMode::Normal {
        let parent_entry = FileEntry::new(
            panel
                .current_path
                .parent()
                .unwrap_or(Path::new("/"))
                .to_path_buf(),
            "..".to_string(),
            EntryKind::Dir,
            0,
        );
        panel.entries.push(parent_entry);
    }

    // List directory contents
    if let Ok(entries) = fs.list_dir(&panel.current_path) {
        let filtered: Vec<FileEntry> = entries
            .into_iter()
            .filter(|e| {
                panel.filter_string.is_empty()
                    || e.name
                        .to_lowercase()
                        .contains(&panel.filter_string.to_lowercase())
            })
            .collect();
        panel.entries.extend(filtered);
    }

    panel.cursor = 0;
    panel.scroll = 0;
}

/// Moves the cursor up or down.
pub fn move_cursor(panel: &mut PanelState, direction: isize, visible_rows: u16) {
    let new_cursor = panel.cursor as isize + direction;
    if new_cursor >= 0 && (new_cursor as usize) < panel.entries.len() {
        panel.cursor = new_cursor as usize;
        if panel.cursor < panel.scroll {
            panel.scroll = panel.cursor;
        } else if panel.cursor >= panel.scroll + visible_rows as usize {
            panel.scroll = panel.cursor - visible_rows as usize + 1;
        }
    }
}

/// Navigates to the home position.
pub fn navigate_home(panel: &mut PanelState) {
    panel.cursor = 0;
    panel.scroll = 0;
}

/// Navigates to the end position.
pub fn navigate_end(panel: &mut PanelState, visible_rows: u16) {
    if panel.entries.is_empty() {
        return;
    }
    panel.cursor = panel.entries.len() - 1;
    panel.scroll = if panel.entries.len() > visible_rows as usize {
        panel.entries.len() - visible_rows as usize
    } else {
        0
    };
}

/// Opens the selected entry (enters directory or triggers file action).
/// Returns true if the selected item is a directory and was entered.
pub fn enter_selected(fs: &dyn FileSystemPort, panel: &mut PanelState) -> bool {
    if panel.entries.is_empty() || panel.cursor >= panel.entries.len() {
        return false;
    }

    let entry = &panel.entries[panel.cursor];

    if entry.name == ".." {
        go_up_one_level(fs, panel);
        return true;
    }

    if entry.is_dir() {
        let new_path = entry.path.clone();
        panel.current_path = new_path;
        panel.mode = PanelMode::Normal;
        panel.filter_string.clear();
        refresh_entries(fs, panel);
        return true;
    }

    false
}

/// Goes up one directory level.
pub fn go_up_one_level(fs: &dyn FileSystemPort, panel: &mut PanelState) {
    panel.mode = PanelMode::Normal;
    panel.filter_string.clear();
    let old_dir = panel.current_path.clone();

    if let Some(parent) = panel.current_path.parent() {
        let old_name = old_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let old_name_with_sep = format!("{}{}", old_name, MAIN_SEPARATOR);

        panel.current_path = parent.to_path_buf();
        refresh_entries(fs, panel);

        // Try to focus on the old directory
        if let Some(pos) = panel
            .entries
            .iter()
            .position(|e| e.name == old_name_with_sep || e.name == old_name)
        {
            panel.cursor = pos;
            if panel.cursor >= panel.scroll + 10 {
                panel.scroll = panel.cursor.saturating_sub(10);
            }
        }
    }
}

/// Changes to a specific directory.
pub fn change_directory(fs: &dyn FileSystemPort, panel: &mut PanelState, path: PathBuf) {
    panel.mode = PanelMode::Normal;
    panel.filter_string.clear();
    panel.current_path = path;
    refresh_entries(fs, panel);
}

/// Replaces panel entries with a list of search results.
pub fn replace_entries_from_search(panel: &mut PanelState, files: Vec<String>, base_path: &Path) {
    panel.entries.clear();
    panel.cursor = 0;
    panel.scroll = 0;
    panel.mode = PanelMode::Normal;
    panel.filter_string.clear();

    for file in files {
        let path = base_path.join(&file);
        let is_dir = file.ends_with(MAIN_SEPARATOR);
        let kind = if is_dir {
            EntryKind::Dir
        } else {
            EntryKind::File
        };
        let entry = FileEntry::new(path, file, kind, 0);
        panel.entries.push(entry);
    }
}
