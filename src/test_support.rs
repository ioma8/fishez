use std::fs;
use std::path::Path;
use std::path::PathBuf;

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
