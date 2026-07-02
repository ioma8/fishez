use crate::application::ports::FileSystemPort;
use crate::domain::FileEntry;
use std::fs;
use std::path::{Path, PathBuf};

pub fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!("{}_{}", prefix, nanos));
    fs::create_dir_all(&path).unwrap();
    path
}

pub struct CwdGuard {
    original: PathBuf,
}

impl CwdGuard {
    pub fn change_to(path: &Path) -> Self {
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        Self { original }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.original).unwrap();
    }
}

pub struct HomeGuard {
    home: Option<std::ffi::OsString>,
    userprofile: Option<std::ffi::OsString>,
}

impl HomeGuard {
    pub fn set(path: &Path) -> Self {
        let guard = Self {
            home: std::env::var_os("HOME"),
            userprofile: std::env::var_os("USERPROFILE"),
        };
        unsafe {
            std::env::set_var("HOME", path);
            std::env::remove_var("USERPROFILE");
        }
        guard
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.home {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
            match &self.userprofile {
                Some(value) => std::env::set_var("USERPROFILE", value),
                None => std::env::remove_var("USERPROFILE"),
            }
        }
    }
}

/// Shared mock for FileSystemPort. Configurable entries and optional list_dir error.
pub struct MockFileSystem {
    pub entries: Vec<FileEntry>,
    pub error: bool,
}

impl MockFileSystem {
    pub fn new() -> Self {
        Self {
            entries: vec![],
            error: false,
        }
    }

    pub fn with_entries(entries: Vec<FileEntry>) -> Self {
        Self {
            entries,
            error: false,
        }
    }

    pub fn with_error() -> Self {
        Self {
            entries: vec![],
            error: true,
        }
    }
}

impl FileSystemPort for MockFileSystem {
    fn list_dir(&self, _path: &Path) -> Result<Vec<FileEntry>, String> {
        if self.error {
            Err("Mock error".to_string())
        } else {
            Ok(self.entries.clone())
        }
    }

    fn delete(&self, _path: &Path) -> Result<(), String> {
        Ok(())
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
