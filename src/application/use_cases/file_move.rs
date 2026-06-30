//! File move use case.

use crate::application::ports::FileSystemPort;
use crate::application::use_cases::file_copy;
use std::fs;
use std::path::{Path, PathBuf};

/// Moves `sources` into `dest_dir`.
/// Tries `rename` first (atomic, same-fs); falls back to copy+delete on cross-device error.
pub fn move_items(
    sources: &[PathBuf],
    dest_dir: &Path,
    fs_port: &impl FileSystemPort,
) -> Result<(), String> {
    for src in sources {
        let name = src
            .file_name()
            .ok_or_else(|| format!("Invalid source path: {}", src.display()))?;
        let dest = dest_dir.join(name);
        move_one(src, &dest, fs_port)?;
    }
    Ok(())
}

fn move_one(src: &Path, dest: &Path, fs_port: &impl FileSystemPort) -> Result<(), String> {
    match fs_port.rename(src, dest) {
        Ok(()) => Ok(()),
        Err(e) if is_cross_device(&e) => {
            // Copy fully, then delete source only on success.
            file_copy::copy_items(&[src.to_path_buf()], dest.parent().unwrap_or(dest), fs_port)?;
            if src.is_dir() {
                fs::remove_dir_all(src).map_err(|e| e.to_string())?;
            } else {
                fs::remove_file(src).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn is_cross_device(e: &std::io::Error) -> bool {
    // EXDEV = 18 on Linux/macOS
    e.raw_os_error() == Some(18)
        || e.kind() == std::io::ErrorKind::CrossesDevices
}
