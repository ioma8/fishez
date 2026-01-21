//! File system adapter - implements FileSystemPort using std::fs and trash crate.

use crate::application::ports::FileSystemPort;
use crate::domain::{EntryKind, FileEntry};
use std::fs;
use std::path::Path;

/// Standard file system adapter.
pub struct StdFileSystem;

impl Default for StdFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl StdFileSystem {
    pub fn new() -> Self {
        Self
    }
}

impl FileSystemPort for StdFileSystem {
    fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>, String> {
        let entries = fs::read_dir(path).map_err(|e| e.to_string())?;

        let mut result = Vec::new();
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let file_path = entry.path();
            let metadata = entry.metadata().ok();
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

            let kind = if is_dir {
                EntryKind::Dir
            } else {
                EntryKind::File
            };

            // Append separator to directory names
            let display_name = if is_dir {
                format!("{}{}", file_name, std::path::MAIN_SEPARATOR)
            } else {
                file_name
            };

            result.push(FileEntry::new(file_path, display_name, kind, size));
        }

        Ok(result)
    }

    fn delete(&self, path: &Path) -> Result<(), String> {
        trash::delete(path).map_err(|e| format!("Error moving to trash: {}", e))
    }

    fn read_file(&self, path: &Path) -> Result<String, String> {
        fs::read_to_string(path).map_err(|e| e.to_string())
    }

    fn is_file(&self, path: &Path) -> bool {
        fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
    }

    fn is_dir(&self, path: &Path) -> bool {
        fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    }
}
