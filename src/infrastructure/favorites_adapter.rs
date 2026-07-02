//! Favorites persistence adapter.

use std::path::{Path, PathBuf};

const CONFIG_DIR: &str = ".fishez";
const FAVORITES_FILE: &str = "favorites.txt";

/// Load favorites from persistent storage.
pub fn load_favorites() -> Vec<String> {
    std::fs::read_to_string(favorites_path())
        .unwrap_or_default()
        .lines()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Save favorites to persistent storage.
pub fn save_favorites(items: &[String]) {
    let path = favorites_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, items.join("\n"));
}

/// Add a path to favorites if not already present.
pub fn add_favorite(items: &mut Vec<String>, path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_string();
    if !items.contains(&path_str) {
        items.push(path_str);
        save_favorites(items);
        true
    } else {
        false
    }
}

fn favorites_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_DIR)
        .join(FAVORITES_FILE)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{CwdGuard, HomeGuard, create_temp_dir};
    use std::fs;
    use std::sync::Mutex;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_load_favorites_missing_file_returns_empty() {
        let _guard = CWD_LOCK.lock().unwrap();
        let base = create_temp_dir("favorites_missing");
        let _cwd = CwdGuard::change_to(&base);
        let _home = HomeGuard::set(&base);

        let favorites = load_favorites();
        assert!(favorites.is_empty());
    }

    #[test]
    fn test_save_and_load_favorites_round_trip() {
        let _guard = CWD_LOCK.lock().unwrap();
        let base = create_temp_dir("favorites_roundtrip");
        let _cwd = CwdGuard::change_to(&base);
        let _home = HomeGuard::set(&base);

        let items = vec!["/tmp/a".to_string(), "/tmp/b".to_string()];
        save_favorites(&items);
        assert!(!base.join("favorites.txt").exists());
        assert!(base.join(".fishez").join("favorites.txt").exists());
        let loaded = load_favorites();
        assert_eq!(loaded, items);
    }

    #[test]
    fn test_add_favorite_de_dupes_and_persists() {
        let _guard = CWD_LOCK.lock().unwrap();
        let base = create_temp_dir("favorites_add");
        let _cwd = CwdGuard::change_to(&base);
        let _home = HomeGuard::set(&base);

        let mut items = vec![];
        let path = Path::new("/tmp/test");
        assert!(add_favorite(&mut items, path));
        assert!(!add_favorite(&mut items, path));
        let loaded = load_favorites();
        assert_eq!(loaded, vec!["/tmp/test".to_string()]);
    }

    #[test]
    fn test_load_favorites_ignores_empty_lines() {
        let _guard = CWD_LOCK.lock().unwrap();
        let base = create_temp_dir("favorites_empty_lines");
        let _cwd = CwdGuard::change_to(&base);
        let _home = HomeGuard::set(&base);

        fs::create_dir(base.join(".fishez")).unwrap();
        fs::write(base.join(".fishez").join("favorites.txt"), "\n/a\n\n/b\n").unwrap();
        let loaded = load_favorites();
        assert_eq!(loaded, vec!["/a".to_string(), "/b".to_string()]);
    }
}
