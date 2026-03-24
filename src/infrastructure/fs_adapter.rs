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
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let size = if is_dir {
                0
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            };

            let kind = if is_dir {
                EntryKind::Dir
            } else {
                EntryKind::File
            };

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

    fn is_dir(&self, path: &Path) -> bool {
        fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn create_temp_dir(prefix: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("{}_{}", prefix, nanos));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_list_dir_formats_entries() {
        let base = create_temp_dir("fs_list");
        let dir_path = base.join("docs");
        let file_path = base.join("file.txt");
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(&file_path, "hello").unwrap();

        let fs_adapter = StdFileSystem::new();
        let entries = fs_adapter.list_dir(&base).unwrap();
        let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
        assert!(names.iter().any(|n| n.starts_with("docs")));
        assert!(names.contains(&"file.txt".to_string()));
        let dir_entry = entries.iter().find(|e| e.name.starts_with("docs")).unwrap();
        assert_eq!(dir_entry.kind, EntryKind::Dir);
        let file_entry = entries.iter().find(|e| e.name == "file.txt").unwrap();
        assert_eq!(file_entry.kind, EntryKind::File);
        assert_eq!(file_entry.size, 5);
    }

    #[test]
    fn test_read_file_error_maps() {
        let fs_adapter = StdFileSystem::new();
        let missing = Path::new("/nonexistent/file.txt");
        let result = fs_adapter.read_file(missing);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_file_and_is_dir() {
        let base = create_temp_dir("fs_is");
        let dir_path = base.join("docs");
        let file_path = base.join("file.txt");
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(&file_path, "hello").unwrap();

        let fs_adapter = StdFileSystem::new();
        assert!(fs_adapter.is_dir(&dir_path));
        assert!(!fs_adapter.is_dir(&file_path));
    }
}
