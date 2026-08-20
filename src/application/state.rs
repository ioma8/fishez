//! Application state - holds the current state of the application.

use crate::domain::FileEntry;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

/// A recursively-computed size figure that may still be in flight on a background thread.
/// The `u64` in `Computing` is the generation token: a result only applies if it matches
/// the panel's current generation counter for that figure, discarding stale results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeFigure {
    Idle,
    Computing(u64),
    Ready(u64),
}

/// The mode of the panel view.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelMode {
    Normal,
    Filter,
    QuickView(QuickViewMode),
}

/// Quick view mode variants.
#[derive(Debug, Clone, PartialEq)]
pub enum QuickViewMode {
    Loading { message: String },
    Text { lines: Vec<String>, start: usize },
    Image(Vec<u8>),
    Directory { lines: Vec<String> },
    NotSupported,
}

/// State of a single panel.
#[derive(Debug)]
pub struct PanelState {
    pub current_path: PathBuf,
    pub entries: Vec<FileEntry>,
    pub cursor: usize,
    pub scroll: usize,
    pub mode: PanelMode,
    pub filter_string: String,
    pub notification: Option<String>,
    pub notification_created: Instant,
    pub multi_selected: HashSet<usize>,
    pub dir_total: SizeFigure,
    pub dir_total_generation: u64,
    pub quick_view_generation: u64,
    pub selection_total: SizeFigure,
    pub selection_total_generation: u64,
    pub selection_cancel: Option<Arc<AtomicBool>>,
    pub show_hidden: bool,
    /// Session-lifetime cache of already-computed directory totals, keyed by path, so
    /// revisiting a directory shows its real size instantly instead of recomputing.
    /// Can go stale if the directory's contents change between visits.
    pub dir_total_cache: HashMap<PathBuf, u64>,
}

impl Default for PanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelState {
    /// Creates a new PanelState with default values.
    pub fn new() -> Self {
        Self {
            current_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            entries: Vec::new(),
            cursor: 0,
            scroll: 0,
            mode: PanelMode::Normal,
            filter_string: String::new(),
            notification: None,
            notification_created: Instant::now(),
            multi_selected: HashSet::new(),
            dir_total: SizeFigure::Idle,
            dir_total_generation: 0,
            quick_view_generation: 0,
            selection_total: SizeFigure::Idle,
            selection_total_generation: 0,
            selection_cancel: None,
            show_hidden: true,
            dir_total_cache: HashMap::new(),
        }
    }

    /// Sets a notification message.
    pub fn set_notification(&mut self, message: String) {
        self.notification = Some(message);
        self.notification_created = Instant::now();
    }

    /// Clears the notification if the timeout has elapsed.
    pub fn clear_notification_if_expired(&mut self, timeout_ms: u64) {
        if self.notification.is_some()
            && self.notification_created.elapsed() > Duration::from_millis(timeout_ms)
        {
            self.notification = None;
        }
    }

    /// Forces the notification to be cleared.
    pub fn clear_notification_force(&mut self) {
        self.notification = None;
    }

    /// Gets the currently selected entry's absolute path.
    pub fn get_selected_path(&self) -> Option<PathBuf> {
        self.selected_entry().map(|entry| entry.path.clone())
    }

    /// Gets the currently selected entry.
    pub fn selected_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.cursor)
    }

    /// Toggles multi-selection for the given index.
    pub fn toggle_multi_selection(&mut self, index: usize) {
        if index < self.entries.len() {
            if self.multi_selected.contains(&index) {
                self.multi_selected.remove(&index);
            } else {
                self.multi_selected.insert(index);
            }
        }
    }

    /// Clears all multi-selections, cancelling any in-flight selection-size computation.
    pub fn clear_multi_selection(&mut self) {
        self.multi_selected.clear();
        if let Some(cancel) = self.selection_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.selection_total = SizeFigure::Idle;
    }

    /// Returns the count of multi-selected items.
    pub fn multi_selected_count(&self) -> usize {
        self.multi_selected.len()
    }

    /// Checks if the given index is multi-selected.
    pub fn is_multi_selected(&self, index: usize) -> bool {
        self.multi_selected.contains(&index)
    }

    /// Gets paths of all multi-selected entries.
    pub fn multi_selected_paths(&self) -> Vec<PathBuf> {
        let mut indexes: Vec<usize> = self.multi_selected.iter().copied().collect();
        indexes.sort_unstable();
        indexes
            .into_iter()
            .filter_map(|idx| self.entries.get(idx))
            .map(|e| e.path.clone())
            .collect()
    }
}

/// Which pane is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    Left,
    Right,
}

/// The overall application state.
#[derive(Debug)]
pub struct AppState {
    pub left_panel: PanelState,
    pub right_panel: PanelState,
    pub active_pane: ActivePane,
    pub show_help: bool,
    pub show_onboarding: bool,
    pub two_pane_mode: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(false)
    }
}

impl AppState {
    /// Creates a new AppState.
    pub fn new(two_pane_mode: bool) -> Self {
        Self {
            left_panel: PanelState::new(),
            right_panel: PanelState::new(),
            active_pane: ActivePane::Left,
            show_help: false,
            show_onboarding: true,
            two_pane_mode,
        }
    }

    /// Gets a mutable reference to the active panel.
    pub fn active_panel_mut(&mut self) -> &mut PanelState {
        match self.active_pane {
            ActivePane::Left => &mut self.left_panel,
            ActivePane::Right => &mut self.right_panel,
        }
    }

    /// Gets a reference to the active panel.
    pub fn active_panel(&self) -> &PanelState {
        match self.active_pane {
            ActivePane::Left => &self.left_panel,
            ActivePane::Right => &self.right_panel,
        }
    }

    /// Switches to the other pane.
    pub fn switch_pane(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::Left => ActivePane::Right,
            ActivePane::Right => ActivePane::Left,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EntryKind;
    use std::thread;
    use std::time::Duration;

    // Helper to create test entries
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

    // PanelState tests
    #[test]
    fn test_panel_state_set_notification() {
        let mut panel = PanelState::new();
        panel.set_notification("Test message".to_string());
        assert_eq!(panel.notification, Some("Test message".to_string()));
    }

    #[test]
    fn test_panel_state_clear_notification_force() {
        let mut panel = PanelState::new();
        panel.set_notification("Test".to_string());
        assert!(panel.notification.is_some());
        panel.clear_notification_force();
        assert!(panel.notification.is_none());
    }

    #[test]
    fn test_panel_state_clear_notification_if_expired_not_expired() {
        let mut panel = PanelState::new();
        panel.set_notification("Test".to_string());
        panel.clear_notification_if_expired(5000);
        assert!(panel.notification.is_some());
    }

    #[test]
    fn test_panel_state_clear_notification_if_expired_expired() {
        let mut panel = PanelState::new();
        panel.set_notification("Test".to_string());
        // Force the notification to be old
        thread::sleep(Duration::from_millis(10));
        panel.clear_notification_if_expired(1);
        assert!(panel.notification.is_none());
    }

    #[test]
    fn test_panel_state_clear_notification_if_expired_no_notification() {
        let mut panel = PanelState::new();
        panel.clear_notification_if_expired(1000);
        assert!(panel.notification.is_none());
    }

    #[test]
    fn test_panel_state_get_selected_path_empty() {
        let panel = PanelState::new();
        assert!(panel.get_selected_path().is_none());
    }

    #[test]
    fn test_panel_state_get_selected_path_with_entries() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 0;
        assert_eq!(
            panel.get_selected_path(),
            Some(PathBuf::from("/home/user/file1.txt"))
        );
    }

    #[test]
    fn test_panel_state_get_selected_path_cursor_at_end() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 2;
        assert_eq!(
            panel.get_selected_path(),
            Some(PathBuf::from("/home/user/docs"))
        );
    }

    #[test]
    fn test_panel_state_get_selected_path_cursor_out_of_bounds() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.cursor = 100;
        assert!(panel.get_selected_path().is_none());
    }

    #[test]
    fn test_panel_state_toggle_multi_selection_add() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.toggle_multi_selection(0);
        assert!(panel.is_multi_selected(0));
        assert!(!panel.is_multi_selected(1));
    }

    #[test]
    fn test_panel_state_toggle_multi_selection_remove() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.toggle_multi_selection(0);
        assert!(panel.is_multi_selected(0));
        panel.toggle_multi_selection(0);
        assert!(!panel.is_multi_selected(0));
    }

    #[test]
    fn test_panel_state_toggle_multi_selection_out_of_bounds() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.toggle_multi_selection(100);
        assert_eq!(panel.multi_selected_count(), 0);
    }

    #[test]
    fn test_panel_state_clear_multi_selection() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.toggle_multi_selection(0);
        panel.toggle_multi_selection(1);
        assert_eq!(panel.multi_selected_count(), 2);
        panel.clear_multi_selection();
        assert_eq!(panel.multi_selected_count(), 0);
    }

    #[test]
    fn test_panel_state_multi_selected_count() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        assert_eq!(panel.multi_selected_count(), 0);
        panel.toggle_multi_selection(0);
        assert_eq!(panel.multi_selected_count(), 1);
        panel.toggle_multi_selection(1);
        assert_eq!(panel.multi_selected_count(), 2);
        panel.toggle_multi_selection(2);
        assert_eq!(panel.multi_selected_count(), 3);
    }

    #[test]
    fn test_panel_state_is_multi_selected() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        assert!(!panel.is_multi_selected(0));
        panel.toggle_multi_selection(0);
        assert!(panel.is_multi_selected(0));
    }

    #[test]
    fn test_panel_state_multi_selected_paths_empty() {
        let panel = PanelState::new();
        assert!(panel.multi_selected_paths().is_empty());
    }

    #[test]
    fn test_panel_state_multi_selected_paths_with_selections() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.toggle_multi_selection(0);
        panel.toggle_multi_selection(2);
        let paths = panel.multi_selected_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/home/user/file1.txt"));
        assert_eq!(paths[1], PathBuf::from("/home/user/docs"));
    }

    #[test]
    fn test_panel_state_multi_selected_paths_sorted() {
        let mut panel = PanelState::new();
        panel.entries = create_test_entries();
        panel.toggle_multi_selection(2);
        panel.toggle_multi_selection(0);
        let paths = panel.multi_selected_paths();
        // Should be sorted by index
        assert_eq!(paths[0], PathBuf::from("/home/user/file1.txt"));
        assert_eq!(paths[1], PathBuf::from("/home/user/docs"));
    }

    // ActivePane and AppState tests
    #[test]
    fn test_app_state_active_panel_mut_left() {
        let mut state = AppState::new(false);
        state.active_pane = ActivePane::Left;
        state.left_panel.cursor = 5;
        let panel = state.active_panel_mut();
        assert_eq!(panel.cursor, 5);
    }

    #[test]
    fn test_app_state_active_panel_mut_right() {
        let mut state = AppState::new(true);
        state.active_pane = ActivePane::Right;
        state.right_panel.cursor = 10;
        let panel = state.active_panel_mut();
        assert_eq!(panel.cursor, 10);
    }

    #[test]
    fn test_app_state_active_panel_left() {
        let mut state = AppState::new(false);
        state.active_pane = ActivePane::Left;
        state.left_panel.cursor = 5;
        let panel = state.active_panel();
        assert_eq!(panel.cursor, 5);
    }

    #[test]
    fn test_app_state_active_panel_right() {
        let mut state = AppState::new(true);
        state.active_pane = ActivePane::Right;
        state.right_panel.cursor = 10;
        let panel = state.active_panel();
        assert_eq!(panel.cursor, 10);
    }

    #[test]
    fn test_app_state_switch_pane_left_to_right() {
        let mut state = AppState::new(true);
        assert_eq!(state.active_pane, ActivePane::Left);
        state.switch_pane();
        assert_eq!(state.active_pane, ActivePane::Right);
    }

    #[test]
    fn test_app_state_switch_pane_right_to_left() {
        let mut state = AppState::new(true);
        state.active_pane = ActivePane::Right;
        state.switch_pane();
        assert_eq!(state.active_pane, ActivePane::Left);
    }

    #[test]
    fn test_app_state_switch_pane_toggle() {
        let mut state = AppState::new(true);
        assert_eq!(state.active_pane, ActivePane::Left);
        state.switch_pane();
        assert_eq!(state.active_pane, ActivePane::Right);
        state.switch_pane();
        assert_eq!(state.active_pane, ActivePane::Left);
    }
}
