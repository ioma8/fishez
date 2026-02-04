//! Favorites persistence adapter.

use std::path::Path;

const FAVORITES_FILE: &str = "favorites.txt";

/// Load favorites from persistent storage.
pub fn load_favorites() -> Vec<String> {
    std::fs::read_to_string(FAVORITES_FILE)
        .unwrap_or_default()
        .lines()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Save favorites to persistent storage.
pub fn save_favorites(items: &[String]) {
    let _ = std::fs::write(FAVORITES_FILE, items.join("\n"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn create_temp_dir(prefix: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("{}_{}", prefix, nanos));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_load_favorites_missing_file_returns_empty() {
        let _guard = CWD_LOCK.lock().unwrap();
        let base = create_temp_dir("favorites_missing");
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base).unwrap();

        let favorites = load_favorites();
        assert!(favorites.is_empty());

        std::env::set_current_dir(original).unwrap();
    }

    #[test]
    fn test_save_and_load_favorites_round_trip() {
        let _guard = CWD_LOCK.lock().unwrap();
        let base = create_temp_dir("favorites_roundtrip");
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base).unwrap();

        let items = vec!["/tmp/a".to_string(), "/tmp/b".to_string()];
        save_favorites(&items);
        let loaded = load_favorites();
        assert_eq!(loaded, items);

        std::env::set_current_dir(original).unwrap();
    }

    #[test]
    fn test_add_favorite_de_dupes_and_persists() {
        let _guard = CWD_LOCK.lock().unwrap();
        let base = create_temp_dir("favorites_add");
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base).unwrap();

        let mut items = vec![];
        let path = Path::new("/tmp/test");
        assert!(add_favorite(&mut items, path));
        assert!(!add_favorite(&mut items, path));
        let loaded = load_favorites();
        assert_eq!(loaded, vec!["/tmp/test".to_string()]);

        std::env::set_current_dir(original).unwrap();
    }

    #[test]
    fn test_load_favorites_ignores_empty_lines() {
        let _guard = CWD_LOCK.lock().unwrap();
        let base = create_temp_dir("favorites_empty_lines");
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&base).unwrap();

        fs::write("favorites.txt", "\n/a\n\n/b\n").unwrap();
        let loaded = load_favorites();
        assert_eq!(loaded, vec!["/a".to_string(), "/b".to_string()]);

        std::env::set_current_dir(original).unwrap();
    }
}
