//! Application ports - interfaces for external dependencies.

use crate::domain::FileEntry;
use std::path::Path;

/// Port for file system operations.
pub trait FileSystemPort {
    /// Lists entries in the specified directory.
    fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>, String>;

    /// Deletes the specified path (moves to trash).
    fn delete(&self, path: &Path) -> Result<(), String>;

    /// Renames (or moves within the same filesystem) a path.
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;

    /// Copies a single file; caller handles directories recursively.
    fn copy_file(&self, from: &Path, to: &Path) -> std::io::Result<()>;

    /// Creates a directory (non-recursive).
    fn create_dir(&self, path: &Path) -> std::io::Result<()>;
}

/// Port for clipboard operations.
pub trait ClipboardPort {
    /// Copies the given text to the clipboard.
    fn copy(&mut self, text: &str) -> Result<(), String>;
}
