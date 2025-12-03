//! Domain entity representing a file or directory entry.

use std::path::PathBuf;

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
    #[allow(dead_code)]
    pub size: u64,
    #[allow(dead_code)]
    pub is_selected: bool,
}

impl FileEntry {
    /// Creates a new FileEntry.
    pub fn new(path: PathBuf, name: String, kind: EntryKind, size: u64) -> Self {
        Self {
            path,
            name,
            kind,
            size,
            is_selected: false,
        }
    }

    /// Checks if this entry represents a directory.
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Dir
    }

    /// Checks if this entry represents a file.
    #[allow(dead_code)]
    pub fn is_file(&self) -> bool {
        self.kind == EntryKind::File
    }
}
