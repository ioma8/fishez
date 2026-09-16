//! File operations use cases - delete, copy, etc.

use crate::application::ports::{ClipboardPort, FileSystemPort};
use crate::application::state::PanelState;
use std::path::Path;

/// Deletes the selected file or directory (moves to trash).
pub fn delete_selected(fs: &dyn FileSystemPort, paths: &[&Path]) -> Result<(), String> {
    for path in paths {
        fs.delete(path)?;
    }
    Ok(())
}

/// Copies the selected file path to clipboard.
pub fn copy_to_clipboard(
    clipboard: &mut dyn ClipboardPort,
    panel: &mut PanelState,
    absolute_path: bool,
) -> Result<(), String> {
    let Some(entry) = panel.selected_entry() else {
        return Err("No file selected".to_string());
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EntryKind, FileEntry};
    use crate::test_support::{MockClipboard, MockFileSystem};
    use std::path::{Path, PathBuf};

    fn create_test_entries() -> Vec<FileEntry> {
        vec![
            FileEntry::new(
                PathBuf::from("/home/user/file1.txt"),
                "file1.txt".to_string(),
                EntryKind::File,
                100,
            ),
            FileEntry::new(
                PathBuf::from("/home/user/file2.txt"),
                "file2.txt".to_string(),
                EntryKind::File,
                200,
            ),
        ]
    }

    // delete_selected tests
    #[test]
    fn test_delete_selected_single() {
        let fs = MockFileSystem::new();
        let path = PathBuf::from("/home/user/file1.txt");
        let paths: Vec<&Path> = vec![path.as_path()];
        let result = delete_selected(&fs, &paths);
        assert!(result.is_ok());
        assert_eq!(fs.deleted_paths(), vec![path]);
    }

    #[test]
    fn test_delete_selected_multiple() {
        let fs = MockFileSystem::new();
        let path1 = PathBuf::from("/home/user/file1.txt");
        let path2 = PathBuf::from("/home/user/file2.txt");
        let paths: Vec<&Path> = vec![path1.as_path(), path2.as_path()];
        let result = delete_selected(&fs, &paths);
        assert!(result.is_ok());
        assert_eq!(fs.deleted_paths(), vec![path1, path2]);
    }

    #[test]
    fn test_delete_selected_error() {
        let fs = MockFileSystem::with_delete_error();
        let path = PathBuf::from("/home/user/file1.txt");
        let paths: Vec<&Path> = vec![path.as_path()];
        let result = delete_selected(&fs, &paths);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Delete failed");
    }

    #[test]
    fn test_delete_selected_empty_paths() {
        let fs = MockFileSystem::new();
        let paths: Vec<&Path> = vec![];
        let result = delete_selected(&fs, &paths);
        assert!(result.is_ok());
    }

    // copy_to_clipboard tests
    #[test]
    fn test_copy_to_clipboard_absolute_path() {
        let mut clipboard = MockClipboard::new();
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 0;
        let result = copy_to_clipboard(&mut clipboard, &mut panel, true);
        assert!(result.is_ok());
        assert_eq!(
            clipboard.content(),
            Some("/home/user/file1.txt".to_string())
        );
        assert!(panel.notification.is_some());
        assert!(
            panel
                .notification
                .as_ref()
                .unwrap()
                .contains("absolute path")
        );
    }

    #[test]
    fn test_copy_to_clipboard_name_only() {
        let mut clipboard = MockClipboard::new();
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 0;
        let result = copy_to_clipboard(&mut clipboard, &mut panel, false);
        assert!(result.is_ok());
        assert_eq!(clipboard.content(), Some("file1.txt".to_string()));
        assert!(panel.notification.is_some());
        assert!(panel.notification.as_ref().unwrap().contains("name"));
    }

    #[test]
    fn test_copy_to_clipboard_empty_entries() {
        let mut clipboard = MockClipboard::new();
        let mut panel = PanelState::new();
        let result = copy_to_clipboard(&mut clipboard, &mut panel, true);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No file selected");
    }

    #[test]
    fn test_copy_to_clipboard_cursor_out_of_bounds() {
        let mut clipboard = MockClipboard::new();
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 100;
        let result = copy_to_clipboard(&mut clipboard, &mut panel, true);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No file selected");
    }

    #[test]
    fn test_copy_to_clipboard_error() {
        let mut clipboard = MockClipboard::with_error();
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 0;
        let result = copy_to_clipboard(&mut clipboard, &mut panel, true);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Clipboard error");
    }

    #[test]
    fn test_copy_to_clipboard_second_file() {
        let mut clipboard = MockClipboard::new();
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 1;
        let result = copy_to_clipboard(&mut clipboard, &mut panel, false);
        assert!(result.is_ok());
        assert_eq!(clipboard.content(), Some("file2.txt".to_string()));
    }
}
