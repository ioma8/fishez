//! Application ports - interfaces for external dependencies.

use crate::domain::FileEntry;
use std::path::Path;

/// Port for file system operations.
pub trait FileSystemPort {
    /// Lists entries in the specified directory.
    fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>, String>;

    /// Deletes the specified path (moves to trash).
    fn delete(&self, path: &Path) -> Result<(), String>;

    /// Reads the content of a file as a string.
    #[allow(dead_code)]
    fn read_file(&self, path: &Path) -> Result<String, String>;

    /// Checks if the given path is a file.
    fn is_file(&self, path: &Path) -> bool;

    /// Checks if the given path is a directory.
    #[allow(dead_code)]
    fn is_dir(&self, path: &Path) -> bool;
}

/// Port for search operations.
pub trait SearchPort {
    /// Finds files matching the query within the given path.
    fn find(&self, query: &str, path: &Path) -> Vec<String>;
}

/// Port for clipboard operations.
pub trait ClipboardPort {
    /// Copies the given text to the clipboard.
    fn copy(&mut self, text: &str) -> Result<(), String>;
}

/// Port for opening files with system default applications.
pub trait OpenPort {
    /// Opens the specified path with the system default application.
    fn open(&self, path: &Path);
}
