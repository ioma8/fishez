//! File system adapter - implements FileSystemPort using std::fs and trash crate.

use crate::application::ports::FileSystemPort;
use crate::domain::{EntryKind, FileEntry};
use std::fs;
use std::path::Path;

/// Standard file system adapter.
#[derive(Default)]
pub struct StdFileSystem;

impl FileSystemPort for StdFileSystem {
    fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>, String> {
        let entries = fs::read_dir(path).map_err(|e| e.to_string())?;

        let mut result = Vec::new();
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let file_path = entry.path();
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            // Directories do not need size/mtime in the listing; avoid a second syscall
            // for them. Files still get one metadata lookup for the size and timestamp.
            let metadata = (!is_dir).then(|| entry.metadata().ok()).flatten();
            let size = if is_dir {
                0
            } else {
                metadata.as_ref().map(|m| m.len()).unwrap_or(0)
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

            let mut file_entry = FileEntry::new(file_path, display_name, kind, size);
            file_entry.modified = metadata.and_then(|m| m.modified().ok());
            result.push(file_entry);
        }

        Ok(result)
    }

    fn delete(&self, path: &Path) -> Result<(), String> {
        trash::delete(path).map_err(|e| format!("Error moving to trash: {}", e))
    }

    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        fs::rename(from, to)
    }

    fn copy_file(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        fs::copy(from, to).map(|_| ())
    }

    fn create_dir(&self, path: &Path) -> std::io::Result<()> {
        fs::create_dir(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::create_temp_dir;
    use std::fs;

    #[test]
    fn test_list_dir_formats_entries() {
        let base = create_temp_dir("fs_list");
        let dir_path = base.join("docs");
        let file_path = base.join("file.txt");
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(&file_path, "hello").unwrap();

        let entries = StdFileSystem.list_dir(&base).unwrap();
        let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
        assert!(names.iter().any(|n| n.starts_with("docs")));
        assert!(names.contains(&"file.txt".to_string()));
        let dir_entry = entries.iter().find(|e| e.name.starts_with("docs")).unwrap();
        assert_eq!(dir_entry.kind, EntryKind::Dir);
        let file_entry = entries.iter().find(|e| e.name == "file.txt").unwrap();
        assert_eq!(file_entry.kind, EntryKind::File);
        assert_eq!(file_entry.size, 5);
    }
}
