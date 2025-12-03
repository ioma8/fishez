//! Application state - holds the current state of the application.

use crate::domain::FileEntry;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    Text {
        lines: Vec<String>,
        start: usize,
        length: usize,
    },
    Image(Vec<u8>, Vec<u8>),
    Directory {
        lines: Vec<String>,
    },
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
        self.entries.get(self.cursor).map(|e| e.path.clone())
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

    /// Clears all multi-selections.
    pub fn clear_multi_selection(&mut self) {
        self.multi_selected.clear();
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
