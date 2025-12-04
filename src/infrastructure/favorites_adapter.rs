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
