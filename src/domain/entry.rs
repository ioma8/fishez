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
    fn test_file_entry_new_creates_file() {
        let entry = FileEntry::new(
            PathBuf::from("/home/user/test.txt"),
            "test.txt".to_string(),
            EntryKind::File,
            1024,
        );
        assert_eq!(entry.path, PathBuf::from("/home/user/test.txt"));
        assert_eq!(entry.name, "test.txt");
        assert_eq!(entry.kind, EntryKind::File);
        assert_eq!(entry.size, 1024);
    }

    #[test]
    fn test_file_entry_new_creates_directory() {
        let entry = FileEntry::new(
            PathBuf::from("/home/user/docs"),
            "docs".to_string(),
            EntryKind::Dir,
            0,
        );
        assert_eq!(entry.path, PathBuf::from("/home/user/docs"));
        assert_eq!(entry.name, "docs");
        assert_eq!(entry.kind, EntryKind::Dir);
        assert_eq!(entry.size, 0);
    }

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

    #[test]
    fn test_entry_kind_equality() {
        assert_eq!(EntryKind::File, EntryKind::File);
        assert_eq!(EntryKind::Dir, EntryKind::Dir);
        assert_ne!(EntryKind::File, EntryKind::Dir);
    }

    #[test]
    fn test_entry_kind_clone() {
        let kind = EntryKind::Dir;
        let cloned = kind;
        assert_eq!(kind, cloned);
    }

    #[test]
    fn test_file_entry_clone() {
        let entry = FileEntry::new(
            PathBuf::from("/home/user/test.txt"),
            "test.txt".to_string(),
            EntryKind::File,
            1024,
        );
        let cloned = entry.clone();
        assert_eq!(entry.path, cloned.path);
        assert_eq!(entry.name, cloned.name);
        assert_eq!(entry.kind, cloned.kind);
        assert_eq!(entry.size, cloned.size);
    }

    #[test]
    fn test_file_entry_with_zero_size() {
        let entry = FileEntry::new(
            PathBuf::from("/home/user/empty.txt"),
            "empty.txt".to_string(),
            EntryKind::File,
            0,
        );
        assert_eq!(entry.size, 0);
    }

    #[test]
    fn test_file_entry_with_large_size() {
        let entry = FileEntry::new(
            PathBuf::from("/home/user/large.bin"),
            "large.bin".to_string(),
            EntryKind::File,
            u64::MAX,
        );
        assert_eq!(entry.size, u64::MAX);
    }

    #[test]
    fn test_file_entry_with_empty_name() {
        let entry = FileEntry::new(
            PathBuf::from("/home/user/"),
            String::new(),
            EntryKind::Dir,
            0,
        );
        assert!(entry.name.is_empty());
    }

    #[test]
    fn test_file_entry_with_unicode_name() {
        let entry = FileEntry::new(
            PathBuf::from("/home/user/文档"),
            "文档".to_string(),
            EntryKind::Dir,
            0,
        );
        assert_eq!(entry.name, "文档");
    }
}
