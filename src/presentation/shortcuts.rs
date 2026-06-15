//! Shortcut handlers - processes keyboard shortcuts.

use crate::application::{AppState, PanelMode};
use crate::infrastructure::{VsCodeAdapter, add_favorite};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;


/// Handle feature shortcuts.
#[allow(clippy::too_many_arguments)]
pub fn handle(
    event: KeyEvent,
    app_state: &mut AppState,
    vscode_adapter: &VsCodeAdapter,
    delete_paths: &mut Option<Vec<PathBuf>>,
    find_filter: &mut Option<String>,
    ripgrep_filter: &mut Option<String>,
    favorites_active: &mut bool,
    favorites_items: &mut Vec<String>,
    favorites_selected: &mut usize,
) -> Option<bool> {
    if let Some(needs_redraw) = handle_delete(event, app_state, delete_paths) {
        return Some(needs_redraw);
    }
    if let Some(needs_redraw) = handle_search(event, find_filter, ripgrep_filter) {
        return Some(needs_redraw);
    }
    if let Some(needs_redraw) = handle_favorites(
        event,
        app_state,
        favorites_active,
        favorites_items,
        favorites_selected,
    ) {
        return Some(needs_redraw);
    }
    if let Some(needs_redraw) = handle_vscode(event, app_state, vscode_adapter) {
        return Some(needs_redraw);
    }
    handle_selection(event, app_state)
}

fn handle_delete(
    event: KeyEvent,
    app_state: &mut AppState,
    delete_paths: &mut Option<Vec<PathBuf>>,
) -> Option<bool> {
    if event.code == KeyCode::Char('w') && event.modifiers.contains(KeyModifiers::CONTROL) {
        let panel = app_state.active_panel_mut();
        let targets = if panel.multi_selected_count() > 0 {
            panel.multi_selected_paths()
        } else {
            panel.get_selected_path().into_iter().collect()
        };
        if !targets.is_empty() {
            *delete_paths = Some(targets);
            return Some(true);
        }
        return Some(false);
    }
    None
}

fn handle_search(
    event: KeyEvent,
    find_filter: &mut Option<String>,
    ripgrep_filter: &mut Option<String>,
) -> Option<bool> {
    if event.code == KeyCode::F(6) {
        *find_filter = Some(String::new());
        return Some(true);
    }
    if event.code == KeyCode::F(7) {
        *ripgrep_filter = Some(String::new());
        return Some(true);
    }
    None
}

fn handle_favorites(
    event: KeyEvent,
    app_state: &mut AppState,
    favorites_active: &mut bool,
    favorites_items: &mut Vec<String>,
    _favorites_selected: &mut usize,
) -> Option<bool> {
    if event.code == KeyCode::Char('d') && event.modifiers.contains(KeyModifiers::CONTROL) {
        let panel = app_state.active_panel_mut();
        if event.modifiers.contains(KeyModifiers::SHIFT) {
            add_favorite(favorites_items, &panel.current_path);
        }
        *favorites_active = true;
        return Some(true);
    }
    None
}

fn handle_vscode(
    event: KeyEvent,
    app_state: &mut AppState,
    vscode_adapter: &VsCodeAdapter,
) -> Option<bool> {
    if event.code == KeyCode::F(4) {
        let panel = app_state.active_panel_mut();
        if let Some(entry) = panel.selected_entry() {
            vscode_adapter.open(&entry.path);
        }
        return Some(false);
    }
    None
}

fn handle_selection(event: KeyEvent, app_state: &mut AppState) -> Option<bool> {
    let panel = app_state.active_panel_mut();

    // Multi-select (Space)
    if event.code == KeyCode::Char(' ') && !matches!(panel.mode, PanelMode::QuickView(_)) {
        let before = panel.multi_selected_count();
        panel.toggle_multi_selection(panel.cursor);
        return Some(panel.multi_selected_count() != before);
    }

    // Clear multi-selection (Esc)
    if event.code == KeyCode::Esc
        && panel.multi_selected_count() > 0
        && !matches!(panel.mode, PanelMode::QuickView(_))
    {
        panel.clear_multi_selection();
        return Some(true);
    }

    None
}
