//! Application ports - interfaces for external dependencies.

use crate::domain::FileEntry;
use std::path::Path;

/// Port for file system operations.
pub trait FileSystemPort {
    /// Lists entries in the specified directory.
    fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>, String>;

    /// Deletes the specified path (moves to trash).
    fn delete(&self, path: &Path) -> Result<(), String>;
}

/// Port for clipboard operations.
pub trait ClipboardPort {
    /// Copies the given text to the clipboard.
    fn copy(&mut self, text: &str) -> Result<(), String>;
}
