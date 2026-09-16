//! Navigation use cases - handles directory navigation and cursor movement.

use crate::application::ports::FileSystemPort;
use crate::application::state::{PanelMode, PanelState, SizeFigure};
use crate::domain::{EntryKind, FileEntry};
use std::path::{MAIN_SEPARATOR, Path, PathBuf};

/// Refreshes the panel entries from the file system.
pub fn refresh_entries(fs: &dyn FileSystemPort, panel: &mut PanelState) {
    panel.clear_multi_selection();
    panel.dir_total = match panel.dir_total_cache.get(&panel.current_path) {
        Some(&cached) => SizeFigure::Ready(cached),
        None => SizeFigure::Idle,
    };
    panel.dir_total_generation += 1;
    panel.directory_entries = fs.list_dir(&panel.current_path).unwrap_or_default();
    sort_entries(&mut panel.directory_entries);
    panel.directory_entry_lower_names = panel
        .directory_entries
        .iter()
        .map(|entry| entry.name.to_lowercase())
        .collect();
    apply_filter(panel);
}

/// Filters the last directory snapshot without touching the filesystem.
/// Each bump invalidates results from workers started for the previous view.
pub fn apply_filter(panel: &mut PanelState) {
    panel.search_generation += 1;
    panel.clear_multi_selection();
    panel.entries.clear();

    // Add parent directory entry unless in filter mode
    if panel.mode == PanelMode::Normal {
        let parent_entry = FileEntry::new(
            panel
                .current_path
                .parent()
                .unwrap_or(Path::new("/"))
                .to_path_buf(),
            "..".to_string(),
            EntryKind::Dir,
            0,
        );
        panel.entries.push(parent_entry);
    }

    let needle = panel.filter_string.to_lowercase();
    panel.entries.extend(
        panel
            .directory_entries
            .iter()
            .zip(&panel.directory_entry_lower_names)
            .filter(|(entry, lower_name)| {
                (panel.show_hidden || !entry.name.starts_with('.'))
                    && (needle.is_empty() || lower_name.contains(&needle))
            })
            .map(|(entry, _)| entry.clone()),
    );

    update_entry_counts(panel);
    panel.cursor = 0;
    panel.scroll = 0;
}

pub fn update_entry_counts(panel: &mut PanelState) {
    let dirs = panel
        .entries
        .iter()
        .filter(|e| e.is_dir() && e.name != "..")
        .count();
    let files = panel.entries.iter().filter(|e| !e.is_dir()).count();
    panel.entry_counts = (dirs, files);
}

fn sort_entries(entries: &mut [FileEntry]) {
    entries.sort_by_cached_key(|entry| {
        (
            entry.name != "..",
            !entry.is_dir(),
            entry.name.to_lowercase(),
        )
    });
}

/// Moves the cursor up or down.
pub fn move_cursor(panel: &mut PanelState, direction: isize, visible_rows: u16) {
    let new_cursor = panel.cursor as isize + direction;
    if new_cursor >= 0 && (new_cursor as usize) < panel.entries.len() {
        panel.cursor = new_cursor as usize;
        if panel.cursor < panel.scroll {
            panel.scroll = panel.cursor;
        } else if panel.cursor >= panel.scroll + visible_rows as usize {
            panel.scroll = panel.cursor - visible_rows as usize + 1;
        }
    }
}

/// Navigates to the home position.
pub fn navigate_home(panel: &mut PanelState) {
    panel.cursor = 0;
    panel.scroll = 0;
}

/// Navigates to the end position.
pub fn navigate_end(panel: &mut PanelState, visible_rows: u16) {
    if panel.entries.is_empty() {
        return;
    }
    panel.cursor = panel.entries.len() - 1;
    panel.scroll = if panel.entries.len() > visible_rows as usize {
        panel.entries.len() - visible_rows as usize
    } else {
        0
    };
}

/// Opens the selected entry (enters directory or triggers file action).
/// Returns true if the selected item is a directory and was entered.
pub fn enter_selected(fs: &dyn FileSystemPort, panel: &mut PanelState) -> bool {
    let Some(entry) = panel.selected_entry() else {
        return false;
    };

    if entry.name == ".." {
        go_up_one_level(fs, panel);
        return true;
    }

    if entry.is_dir() {
        let new_path = entry.path.clone();
        panel.current_path = new_path;
        panel.mode = PanelMode::Normal;
        panel.filter_string.clear();
        refresh_entries(fs, panel);
        return true;
    }

    false
}

/// Goes up one directory level.
pub fn go_up_one_level(fs: &dyn FileSystemPort, panel: &mut PanelState) {
    panel.mode = PanelMode::Normal;
    panel.filter_string.clear();
    let old_dir = panel.current_path.clone();

    if let Some(parent) = panel.current_path.parent() {
        let old_name = old_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let old_name_with_sep = format!("{}{}", old_name, MAIN_SEPARATOR);

        panel.current_path = parent.to_path_buf();
        refresh_entries(fs, panel);

        // Try to focus on the old directory
        if let Some(pos) = panel
            .entries
            .iter()
            .position(|e| e.name == old_name_with_sep || e.name == old_name)
        {
            panel.cursor = pos;
            if panel.cursor >= panel.scroll + 10 {
                panel.scroll = panel.cursor.saturating_sub(10);
            }
        }
    }
}

/// Changes to a specific directory.
pub fn change_directory(fs: &dyn FileSystemPort, panel: &mut PanelState, path: PathBuf) {
    panel.mode = PanelMode::Normal;
    panel.filter_string.clear();
    panel.current_path = path;
    refresh_entries(fs, panel);
}

/// Replaces panel entries with a list of search results.
pub fn replace_entries_from_search(panel: &mut PanelState, files: Vec<String>, base_path: &Path) {
    panel.entries.clear();
    panel.cursor = 0;
    panel.scroll = 0;
    panel.mode = PanelMode::Normal;
    panel.filter_string.clear();

    for file in files {
        let path = base_path.join(&file);
        let is_dir = file.ends_with(MAIN_SEPARATOR);
        let kind = if is_dir {
            EntryKind::Dir
        } else {
            EntryKind::File
        };
        let entry = FileEntry::new(path, file, kind, 0);
        panel.entries.push(entry);
    }
    update_entry_counts(panel);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::MockFileSystem;

    #[test]
    fn test_filter_backspace_restores_snapshot_without_reading_disk() {
        let fs = MockFileSystem::with_entries(create_test_entries());
        let mut panel = PanelState::new();
        refresh_entries(&fs, &mut panel);
        let count = panel.entries.len();
        panel.mode = PanelMode::Filter;
        panel.filter_string = "file1".into();
        apply_filter(&mut panel);
        assert_eq!(panel.entries.len(), 1);
        panel.filter_string.clear();
        panel.mode = PanelMode::Normal;
        apply_filter(&mut panel);
        assert_eq!(panel.entries.len(), count);
        assert_eq!(panel.entry_counts.0 + panel.entry_counts.1 + 1, count);
    }

    fn create_test_entries() -> Vec<FileEntry> {
        vec![
            FileEntry::new(
                PathBuf::from("/home/user/file1.txt"),
                "file1.txt".to_string(),
                EntryKind::File,
                100,
            ),
            FileEntry::new(
                PathBuf::from("/home/user/file2.txt"),
                "file2.txt".to_string(),
                EntryKind::File,
                200,
            ),
            FileEntry::new(
                PathBuf::from("/home/user/docs"),
                "docs".to_string(),
                EntryKind::Dir,
                0,
            ),
        ]
    }

    // move_cursor tests
    #[test]
    fn test_move_cursor_down() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 0;
        move_cursor(&mut panel, 1, 10);
        assert_eq!(panel.cursor, 1);
    }

    #[test]
    fn test_move_cursor_up() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 2;
        move_cursor(&mut panel, -1, 10);
        assert_eq!(panel.cursor, 1);
    }

    #[test]
    fn test_move_cursor_at_beginning() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 0;
        move_cursor(&mut panel, -1, 10);
        assert_eq!(panel.cursor, 0); // Should not go negative
    }

    #[test]
    fn test_move_cursor_at_end() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 2;
        move_cursor(&mut panel, 1, 10);
        assert_eq!(panel.cursor, 2); // Should not exceed entries
    }

    #[test]
    fn test_move_cursor_scroll_up() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 1;
        panel.scroll = 1;
        move_cursor(&mut panel, -1, 10);
        assert_eq!(panel.cursor, 0);
        assert_eq!(panel.scroll, 0);
    }

    #[test]
    fn test_move_cursor_scroll_down() {
        let mut panel = PanelState::new();
        // Create many entries
        for i in 0..20 {
            panel.entries.push(FileEntry::new(
                PathBuf::from(format!("/file{}.txt", i)),
                format!("file{}.txt", i),
                EntryKind::File,
                0,
            ));
        }
        panel.cursor = 9;
        panel.scroll = 0;
        move_cursor(&mut panel, 1, 10);
        assert_eq!(panel.cursor, 10);
        assert_eq!(panel.scroll, 1);
    }

    // navigate_home tests
    #[test]
    fn test_navigate_home() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 2;
        panel.scroll = 1;
        navigate_home(&mut panel);
        assert_eq!(panel.cursor, 0);
        assert_eq!(panel.scroll, 0);
    }

    #[test]
    fn test_navigate_home_already_at_home() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 0;
        panel.scroll = 0;
        navigate_home(&mut panel);
        assert_eq!(panel.cursor, 0);
        assert_eq!(panel.scroll, 0);
    }

    // navigate_end tests
    #[test]
    fn test_navigate_end() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 0;
        navigate_end(&mut panel, 10);
        assert_eq!(panel.cursor, 2);
        assert_eq!(panel.scroll, 0);
    }

    #[test]
    fn test_navigate_end_empty_entries() {
        let mut panel = PanelState::new();
        navigate_end(&mut panel, 10);
        assert_eq!(panel.cursor, 0);
    }

    #[test]
    fn test_navigate_end_with_scroll() {
        let mut panel = PanelState::new();
        for i in 0..20 {
            panel.entries.push(FileEntry::new(
                PathBuf::from(format!("/file{}.txt", i)),
                format!("file{}.txt", i),
                EntryKind::File,
                0,
            ));
        }
        panel.cursor = 0;
        navigate_end(&mut panel, 10);
        assert_eq!(panel.cursor, 19);
        assert_eq!(panel.scroll, 10);
    }

    #[test]
    fn test_navigate_end_entries_fit_in_view() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries(); // 3 entries
        navigate_end(&mut panel, 10); // 10 visible rows
        assert_eq!(panel.cursor, 2);
        assert_eq!(panel.scroll, 0);
    }

    // refresh_entries tests
    #[test]
    fn test_refresh_entries_normal_mode() {
        let fs = MockFileSystem::with_entries(create_test_entries());
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        panel.mode = PanelMode::Normal;
        refresh_entries(&fs, &mut panel);
        // Should have ".." entry plus 3 mock entries
        assert_eq!(panel.entries.len(), 4);
        assert_eq!(panel.entries[0].name, "..");
    }

    #[test]
    fn test_refresh_entries_hides_dotfiles_when_disabled() {
        let fs = MockFileSystem::with_entries(vec![
            FileEntry::new(
                PathBuf::from("/home/user/.env"),
                ".env".to_string(),
                EntryKind::File,
                10,
            ),
            FileEntry::new(
                PathBuf::from("/home/user/visible.txt"),
                "visible.txt".to_string(),
                EntryKind::File,
                10,
            ),
        ]);
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");

        panel.show_hidden = false;
        refresh_entries(&fs, &mut panel);
        assert!(!panel.entries.iter().any(|entry| entry.name == ".env"));
        assert!(
            panel
                .entries
                .iter()
                .any(|entry| entry.name == "visible.txt")
        );

        panel.show_hidden = true;
        refresh_entries(&fs, &mut panel);
        assert!(panel.entries.iter().any(|entry| entry.name == ".env"));
    }

    #[test]
    fn test_refresh_entries_sorts_dirs_first_case_insensitive() {
        let fs = MockFileSystem::with_entries(vec![
            FileEntry::new(
                PathBuf::from("/home/user/zeta.txt"),
                "zeta.txt".to_string(),
                EntryKind::File,
                10,
            ),
            FileEntry::new(
                PathBuf::from("/home/user/Beta"),
                "Beta".to_string(),
                EntryKind::Dir,
                0,
            ),
            FileEntry::new(
                PathBuf::from("/home/user/alpha.txt"),
                "alpha.txt".to_string(),
                EntryKind::File,
                10,
            ),
            FileEntry::new(
                PathBuf::from("/home/user/Alpha"),
                "Alpha".to_string(),
                EntryKind::Dir,
                0,
            ),
        ]);
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");

        refresh_entries(&fs, &mut panel);

        let names: Vec<_> = panel
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, vec!["..", "Alpha", "Beta", "alpha.txt", "zeta.txt"]);
    }

    #[test]
    fn test_refresh_entries_filter_mode() {
        let fs = MockFileSystem::with_entries(create_test_entries());
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        panel.mode = PanelMode::Filter;
        refresh_entries(&fs, &mut panel);
        // Should NOT have ".." entry in filter mode
        assert_eq!(panel.entries.len(), 3);
    }

    #[test]
    fn test_refresh_entries_with_filter_string() {
        let fs = MockFileSystem::with_entries(create_test_entries());
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        panel.filter_string = "file1".to_string();
        refresh_entries(&fs, &mut panel);
        // Should have ".." plus only file1.txt
        assert_eq!(panel.entries.len(), 2);
    }

    #[test]
    fn test_refresh_entries_clears_multi_selected() {
        let fs = MockFileSystem::with_entries(create_test_entries());
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        panel.entries = create_test_entries();
        panel.toggle_multi_selection(0);
        assert_eq!(panel.multi_selected_count(), 1);
        refresh_entries(&fs, &mut panel);
        assert_eq!(panel.multi_selected_count(), 0);
    }

    #[test]
    fn test_refresh_entries_invalidates_cached_dir_total() {
        let fs = MockFileSystem::with_entries(create_test_entries());
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        panel.dir_total = SizeFigure::Ready(12345);
        panel.dir_total_generation = 7;
        refresh_entries(&fs, &mut panel);
        assert_eq!(panel.dir_total, SizeFigure::Idle);
        assert_eq!(panel.dir_total_generation, 8);
    }

    #[test]
    fn test_refresh_entries_restores_cached_dir_total_for_a_revisited_path() {
        let fs = MockFileSystem::with_entries(create_test_entries());
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        panel
            .dir_total_cache
            .insert(PathBuf::from("/home/user"), 999_999);
        // No prior Ready value on the panel itself - simulates arriving fresh at a
        // path this session has already computed a total for.
        refresh_entries(&fs, &mut panel);
        assert_eq!(panel.dir_total, SizeFigure::Ready(999_999));
    }

    #[test]
    fn test_refresh_entries_does_not_use_cache_from_a_different_path() {
        let fs = MockFileSystem::with_entries(create_test_entries());
        let mut panel = PanelState::new();
        panel
            .dir_total_cache
            .insert(PathBuf::from("/some/other/dir"), 999_999);
        panel.current_path = PathBuf::from("/home/user");
        refresh_entries(&fs, &mut panel);
        assert_eq!(panel.dir_total, SizeFigure::Idle);
    }

    #[test]
    fn test_refresh_entries_resets_cursor() {
        let fs = MockFileSystem::with_entries(create_test_entries());
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        panel.cursor = 5;
        panel.scroll = 3;
        refresh_entries(&fs, &mut panel);
        assert_eq!(panel.cursor, 0);
        assert_eq!(panel.scroll, 0);
    }

    #[test]
    fn test_refresh_entries_with_fs_error() {
        let fs = MockFileSystem::with_error();
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        refresh_entries(&fs, &mut panel);
        // Should only have ".." entry
        assert_eq!(panel.entries.len(), 1);
        assert_eq!(panel.entries[0].name, "..");
    }

    // enter_selected tests
    #[test]
    fn test_enter_selected_empty_entries() {
        let fs = MockFileSystem::new();
        let mut panel = PanelState::new();
        let result = enter_selected(&fs, &mut panel);
        assert!(!result);
    }

    #[test]
    fn test_enter_selected_cursor_out_of_bounds() {
        let fs = MockFileSystem::new();
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 100;
        let result = enter_selected(&fs, &mut panel);
        assert!(!result);
    }

    #[test]
    fn test_enter_selected_file() {
        let fs = MockFileSystem::with_entries(vec![]);
        let mut panel = PanelState::new();
        panel.entries = vec![FileEntry::new(
            PathBuf::from("/home/user/file.txt"),
            "file.txt".to_string(),
            EntryKind::File,
            100,
        )];
        panel.cursor = 0;
        let result = enter_selected(&fs, &mut panel);
        assert!(!result); // Files don't get "entered"
    }

    #[test]
    fn test_enter_selected_directory() {
        let fs = MockFileSystem::with_entries(vec![]);
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        panel.entries = vec![FileEntry::new(
            PathBuf::from("/home/user/docs"),
            "docs".to_string(),
            EntryKind::Dir,
            0,
        )];
        panel.cursor = 0;
        let result = enter_selected(&fs, &mut panel);
        assert!(result);
        assert_eq!(panel.current_path, PathBuf::from("/home/user/docs"));
    }

    #[test]
    fn test_enter_selected_clears_filter() {
        let fs = MockFileSystem::with_entries(vec![]);
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        panel.entries = vec![FileEntry::new(
            PathBuf::from("/home/user/docs"),
            "docs".to_string(),
            EntryKind::Dir,
            0,
        )];
        panel.filter_string = "test".to_string();
        panel.mode = PanelMode::Filter;
        panel.cursor = 0;
        enter_selected(&fs, &mut panel);
        assert!(panel.filter_string.is_empty());
        assert_eq!(panel.mode, PanelMode::Normal);
    }

    // go_up_one_level tests
    #[test]
    fn test_go_up_one_level() {
        let fs = MockFileSystem::with_entries(vec![]);
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user/docs");
        go_up_one_level(&fs, &mut panel);
        assert_eq!(panel.current_path, PathBuf::from("/home/user"));
    }

    #[test]
    fn test_go_up_one_level_clears_filter() {
        let fs = MockFileSystem::with_entries(vec![]);
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user/docs");
        panel.filter_string = "test".to_string();
        panel.mode = PanelMode::Filter;
        go_up_one_level(&fs, &mut panel);
        assert!(panel.filter_string.is_empty());
        assert_eq!(panel.mode, PanelMode::Normal);
    }

    // change_directory tests
    #[test]
    fn test_change_directory() {
        let fs = MockFileSystem::with_entries(vec![]);
        let mut panel = PanelState::new();
        panel.current_path = PathBuf::from("/home/user");
        change_directory(&fs, &mut panel, PathBuf::from("/tmp"));
        assert_eq!(panel.current_path, PathBuf::from("/tmp"));
    }

    #[test]
    fn test_change_directory_clears_filter() {
        let fs = MockFileSystem::with_entries(vec![]);
        let mut panel = PanelState::new();
        panel.filter_string = "test".to_string();
        panel.mode = PanelMode::Filter;
        change_directory(&fs, &mut panel, PathBuf::from("/tmp"));
        assert!(panel.filter_string.is_empty());
        assert_eq!(panel.mode, PanelMode::Normal);
    }

    // replace_entries_from_search tests
    #[test]
    fn test_replace_entries_from_search_empty() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        replace_entries_from_search(&mut panel, vec![], Path::new("/home"));
        assert!(panel.entries.is_empty());
    }

    #[test]
    fn test_replace_entries_from_search_files() {
        let mut panel = PanelState::new();
        let files = vec!["file1.txt".to_string(), "file2.txt".to_string()];
        replace_entries_from_search(&mut panel, files, Path::new("/home"));
        assert_eq!(panel.entries.len(), 2);
        assert_eq!(panel.entries[0].name, "file1.txt");
        assert_eq!(panel.entries[0].kind, EntryKind::File);
        assert_eq!(panel.entries[0].path, PathBuf::from("/home/file1.txt"));
    }

    #[test]
    fn test_replace_entries_from_search_directories() {
        let mut panel = PanelState::new();
        let files = vec![format!("docs{}", MAIN_SEPARATOR)];
        replace_entries_from_search(&mut panel, files, Path::new("/home"));
        assert_eq!(panel.entries.len(), 1);
        assert_eq!(panel.entries[0].kind, EntryKind::Dir);
    }

    #[test]
    fn test_replace_entries_from_search_resets_state() {
        let mut panel = PanelState::new();
        panel.cursor = 5;
        panel.scroll = 3;
        panel.filter_string = "test".to_string();
        panel.mode = PanelMode::Filter;
        replace_entries_from_search(&mut panel, vec![], Path::new("/home"));
        assert_eq!(panel.cursor, 0);
        assert_eq!(panel.scroll, 0);
        assert!(panel.filter_string.is_empty());
        assert_eq!(panel.mode, PanelMode::Normal);
    }
}
