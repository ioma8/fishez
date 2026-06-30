//! New folder use case.

use crate::application::ports::FileSystemPort;
use std::path::Path;

/// Creates a new directory named `name` inside `parent`.
pub fn create_folder(
    name: &str,
    parent: &Path,
    fs_port: &impl FileSystemPort,
) -> Result<(), String> {
    let path = parent.join(name);
    fs_port.create_dir(&path).map_err(|e| e.to_string())
}
