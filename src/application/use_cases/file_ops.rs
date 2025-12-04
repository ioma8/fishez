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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::FileSystemPort;
    use crate::domain::{EntryKind, FileEntry};
    use std::path::PathBuf;

    /// Mock file system for testing
    struct MockFileSystem {
        delete_error: bool,
        deleted_paths: std::cell::RefCell<Vec<PathBuf>>,
    }

    impl MockFileSystem {
        fn new() -> Self {
            Self {
                delete_error: false,
                deleted_paths: std::cell::RefCell::new(vec![]),
            }
        }

        fn with_delete_error() -> Self {
            Self {
                delete_error: true,
                deleted_paths: std::cell::RefCell::new(vec![]),
            }
        }

        #[allow(dead_code)]
        fn get_deleted_paths(&self) -> Vec<PathBuf> {
            self.deleted_paths.borrow().clone()
        }
    }

    impl FileSystemPort for MockFileSystem {
        fn list_dir(&self, _path: &Path) -> Result<Vec<FileEntry>, String> {
            Ok(vec![])
        }

        fn delete(&self, path: &Path) -> Result<(), String> {
            if self.delete_error {
                Err("Delete failed".to_string())
            } else {
                self.deleted_paths.borrow_mut().push(path.to_path_buf());
                Ok(())
            }
        }

        fn read_file(&self, _path: &Path) -> Result<String, String> {
            Ok(String::new())
        }

        fn is_file(&self, _path: &Path) -> bool {
            true
        }

        fn is_dir(&self, _path: &Path) -> bool {
            false
        }
    }

    /// Mock clipboard for testing
    struct MockClipboard {
        content: std::cell::RefCell<Option<String>>,
        error: bool,
    }

    impl MockClipboard {
        fn new() -> Self {
            Self {
                content: std::cell::RefCell::new(None),
                error: false,
            }
        }

        fn with_error() -> Self {
            Self {
                content: std::cell::RefCell::new(None),
                error: true,
            }
        }

        fn get_content(&self) -> Option<String> {
            self.content.borrow().clone()
        }
    }

    impl ClipboardPort for MockClipboard {
        fn copy(&mut self, text: &str) -> Result<(), String> {
            if self.error {
                Err("Clipboard error".to_string())
            } else {
                *self.content.borrow_mut() = Some(text.to_string());
                Ok(())
            }
        }
    }

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
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        let path = PathBuf::from("/home/user/file1.txt");
        let paths: Vec<&Path> = vec![path.as_path()];
        let result = delete_selected(&fs, &mut panel, &paths);
        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_selected_multiple() {
        let fs = MockFileSystem::new();
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        let path1 = PathBuf::from("/home/user/file1.txt");
        let path2 = PathBuf::from("/home/user/file2.txt");
        let paths: Vec<&Path> = vec![path1.as_path(), path2.as_path()];
        let result = delete_selected(&fs, &mut panel, &paths);
        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_selected_clears_multi_selection() {
        let fs = MockFileSystem::new();
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        panel.entries = create_test_entries();
        panel.toggle_multi_selection(0);
        panel.toggle_multi_selection(1);
        assert_eq!(panel.multi_selected_count(), 2);
        let paths: Vec<&Path> = vec![];
        delete_selected(&fs, &mut panel, &paths).unwrap();
        assert_eq!(panel.multi_selected_count(), 0);
    }

    #[test]
    fn test_delete_selected_error() {
        let fs = MockFileSystem::with_delete_error();
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        let path = PathBuf::from("/home/user/file1.txt");
        let paths: Vec<&Path> = vec![path.as_path()];
        let result = delete_selected(&fs, &mut panel, &paths);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Delete failed");
    }

    #[test]
    fn test_delete_selected_empty_paths() {
        let fs = MockFileSystem::new();
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        let paths: Vec<&Path> = vec![];
        let result = delete_selected(&fs, &mut panel, &paths);
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
            clipboard.get_content(),
            Some("/home/user/file1.txt".to_string())
        );
        assert!(panel.notification.is_some());
        assert!(panel
            .notification
            .as_ref()
            .unwrap()
            .contains("absolute path"));
    }

    #[test]
    fn test_copy_to_clipboard_name_only() {
        let mut clipboard = MockClipboard::new();
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 0;
        let result = copy_to_clipboard(&mut clipboard, &mut panel, false);
        assert!(result.is_ok());
        assert_eq!(clipboard.get_content(), Some("file1.txt".to_string()));
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
        assert_eq!(clipboard.get_content(), Some("file2.txt".to_string()));
    }
}
