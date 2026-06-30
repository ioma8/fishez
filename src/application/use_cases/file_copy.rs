//! File copy use case.

use crate::application::ports::FileSystemPort;
use std::fs;
use std::path::{Path, PathBuf};

/// Copies `sources` into `dest_dir`. Directories are copied recursively.
pub fn copy_items(
    sources: &[PathBuf],
    dest_dir: &Path,
    fs_port: &impl FileSystemPort,
) -> Result<(), String> {
    for src in sources {
        let name = src
            .file_name()
            .ok_or_else(|| format!("Invalid source path: {}", src.display()))?;
        let dest = dest_dir.join(name);
        copy_one(src, &dest, fs_port)?;
    }
    Ok(())
}

fn copy_one(src: &Path, dest: &Path, fs_port: &impl FileSystemPort) -> Result<(), String> {
    if src.is_dir() {
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
            let child_dest = dest.join(entry.file_name());
            copy_one(&entry.path(), &child_dest, fs_port)?;
        }
    } else {
        fs_port.copy_file(src, dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}
