use crate::application::ports::{ClipboardPort, FileSystemPort};
use crate::domain::FileEntry;
use std::cell::RefCell;
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

/// Shared mock for FileSystemPort. Configurable entries, optional list_dir error,
/// and optional delete failure with a recording of deleted paths.
#[derive(Default)]
pub struct MockFileSystem {
    pub entries: Vec<FileEntry>,
    pub error: bool,
    pub delete_error: bool,
    deleted_paths: RefCell<Vec<PathBuf>>,
}

impl MockFileSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entries(entries: Vec<FileEntry>) -> Self {
        Self {
            entries,
            ..Default::default()
        }
    }

    pub fn with_error() -> Self {
        Self {
            error: true,
            ..Default::default()
        }
    }

    pub fn with_delete_error() -> Self {
        Self {
            delete_error: true,
            ..Default::default()
        }
    }

    /// Paths that `delete` was asked to remove, in call order.
    pub fn deleted_paths(&self) -> Vec<PathBuf> {
        self.deleted_paths.borrow().clone()
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

    fn delete(&self, path: &Path) -> Result<(), String> {
        if self.delete_error {
            Err("Delete failed".to_string())
        } else {
            self.deleted_paths.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
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

/// Mock clipboard for ClipboardPort tests.
#[derive(Default)]
pub struct MockClipboard {
    content: RefCell<Option<String>>,
    error: bool,
}

impl MockClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_error() -> Self {
        Self {
            error: true,
            ..Default::default()
        }
    }

    /// The last text `copy` was asked to write, if any.
    pub fn content(&self) -> Option<String> {
        self.content.borrow().clone()
    }
}

impl ClipboardPort for MockClipboard {
    fn copy(&mut self, text: &str) -> Result<(), String> {
        if self.error {
            Err("Clipboard error".to_string())
        } else {
            *self.content.borrow_mut() = Some(text.to_string());
            Ok(())
        }
    }
}
