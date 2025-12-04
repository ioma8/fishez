//! Shortcut handlers - processes keyboard shortcuts.

use crate::application::{AppState, PanelMode};
use crate::infrastructure::{VsCodeAdapter, add_favorite};
use crate::presentation::TerminalRenderer;
use crate::presentation::terminal::overlays;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

use crate::application::ports::OpenPort;

/// Handle feature shortcuts.
#[allow(clippy::too_many_arguments)]
pub fn handle(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    vscode_adapter: &VsCodeAdapter,
    delete_paths: &mut Option<Vec<PathBuf>>,
    find_filter: &mut Option<String>,
    ripgrep_filter: &mut Option<String>,
    favorites_active: &mut bool,
    favorites_items: &mut Vec<String>,
    favorites_selected: &mut usize,
) -> bool {
    if handle_delete(event, app_state, renderer, delete_paths) {
        return true;
    }
    if handle_search(event, renderer, app_state, find_filter, ripgrep_filter) {
        return true;
    }
    if handle_favorites(
        event,
        app_state,
        renderer,
        favorites_active,
        favorites_items,
        favorites_selected,
    ) {
        return true;
    }
    if handle_vscode(event, app_state, renderer, vscode_adapter) {
        return true;
    }
    handle_selection(event, app_state, renderer)
}

fn handle_delete(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    delete_paths: &mut Option<Vec<PathBuf>>,
) -> bool {
    if event.code == KeyCode::Char('w') && event.modifiers.contains(KeyModifiers::CONTROL) {
        let panel = app_state.active_panel_mut();
        let targets = if panel.multi_selected_count() > 0 {
            panel.multi_selected_paths()
        } else {
            panel.get_selected_path().into_iter().collect()
        };
        if !targets.is_empty() {
            *delete_paths = Some(targets);
        }
        overlays::draw_with_delete(renderer, app_state, delete_paths.as_ref());
        return true;
    }
    false
}

fn handle_search(
    event: KeyEvent,
    renderer: &mut TerminalRenderer,
    app_state: &AppState,
    find_filter: &mut Option<String>,
    ripgrep_filter: &mut Option<String>,
) -> bool {
    if event.code == KeyCode::F(6) {
        *find_filter = Some(String::new());
        overlays::draw_with_find(renderer, app_state, find_filter.as_ref());
        return true;
    }
    if event.code == KeyCode::F(7) {
        *ripgrep_filter = Some(String::new());
        overlays::draw_with_ripgrep(renderer, app_state, ripgrep_filter.as_ref());
        return true;
    }
    false
}

fn handle_favorites(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    favorites_active: &mut bool,
    favorites_items: &mut Vec<String>,
    favorites_selected: &mut usize,
) -> bool {
    if event.code == KeyCode::Char('d') && event.modifiers.contains(KeyModifiers::CONTROL) {
        let panel = app_state.active_panel_mut();
        if event.modifiers.contains(KeyModifiers::SHIFT) {
            add_favorite(favorites_items, &panel.current_path);
        }
        *favorites_active = true;
        overlays::draw_with_favorites(
            renderer,
            app_state,
            *favorites_active,
            favorites_items,
            *favorites_selected,
        );
        return true;
    }
    false
}

fn handle_vscode(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    vscode_adapter: &VsCodeAdapter,
) -> bool {
    if event.code == KeyCode::F(4) {
        let panel = app_state.active_panel_mut();
        if let Some(path) = panel.get_selected_path() {
            vscode_adapter.open(&path);
        }
        overlays::draw(renderer, app_state);
        return true;
    }
    false
}

fn handle_selection(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
) -> bool {
    let panel = app_state.active_panel_mut();

    // Multi-select (Space)
    if event.code == KeyCode::Char(' ') && !matches!(panel.mode, PanelMode::QuickView(_)) {
        panel.toggle_multi_selection(panel.cursor);
        overlays::draw(renderer, app_state);
        return true;
    }

    // Clear multi-selection (Esc)
    if event.code == KeyCode::Esc
        && panel.multi_selected_count() > 0
        && !matches!(panel.mode, PanelMode::QuickView(_))
    {
        panel.clear_multi_selection();
        overlays::draw(renderer, app_state);
        return true;
    }

    false
}
