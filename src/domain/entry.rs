//! Domain entity representing a file or directory entry.

use std::path::PathBuf;
use std::time::SystemTime;

/// Represents the kind of file system entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
}

/// A file system entry containing core file/directory information.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

impl FileEntry {
    /// Creates a new FileEntry.
    pub fn new(path: PathBuf, name: String, kind: EntryKind, size: u64) -> Self {
        Self {
            path,
            name,
            kind,
            size,
            modified: None,
        }
    }

    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Dir
    }

    pub fn is_file(&self) -> bool {
        self.kind == EntryKind::File
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_entry_is_dir() {
        let dir = FileEntry::new(
            PathBuf::from("/home/user/docs"),
            "docs".to_string(),
            EntryKind::Dir,
            0,
        );
        let file = FileEntry::new(
            PathBuf::from("/home/user/test.txt"),
            "test.txt".to_string(),
            EntryKind::File,
            1024,
        );
        assert!(dir.is_dir());
        assert!(!file.is_dir());
    }

    #[test]
    fn test_file_entry_is_file() {
        let dir = FileEntry::new(
            PathBuf::from("/home/user/docs"),
            "docs".to_string(),
            EntryKind::Dir,
            0,
        );
        let file = FileEntry::new(
            PathBuf::from("/home/user/test.txt"),
            "test.txt".to_string(),
            EntryKind::File,
            1024,
        );
        assert!(!dir.is_file());
        assert!(file.is_file());
    }
}
