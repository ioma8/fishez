//! Shortcut handlers - processes keyboard shortcuts.

use crate::application::{AppState, PanelMode};
use crate::infrastructure::{VsCodeAdapter, add_favorite};
use crate::presentation::TerminalRenderer;
use crate::presentation::input_handler::{CopyMoveState, Message, UiState, schedule_size_jobs};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::mpsc::Sender;
use tui_input::Input;

/// Handle feature shortcuts.
pub fn handle(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    vscode_adapter: &VsCodeAdapter,
    ui: &mut UiState,
    sender: &Sender<Message>,
) -> Option<bool> {
    if let Some(needs_redraw) = handle_delete(event, app_state, ui) {
        return Some(needs_redraw);
    }
    if let Some(needs_redraw) = handle_file_ops(event, app_state, ui) {
        return Some(needs_redraw);
    }
    if let Some(needs_redraw) = handle_search(event, ui) {
        return Some(needs_redraw);
    }
    if let Some(needs_redraw) = handle_favorites(event, app_state, ui) {
        return Some(needs_redraw);
    }
    if event.code == KeyCode::Char('j') && event.modifiers.contains(KeyModifiers::CONTROL) {
        ui.recent_active = true;
        ui.recent_selected = 0;
        return Some(true);
    }
    if let Some(needs_redraw) = handle_two_pane_toggle(event, app_state) {
        return Some(needs_redraw);
    }
    if let Some(needs_redraw) = handle_vscode(event, app_state, renderer, vscode_adapter) {
        return Some(needs_redraw);
    }
    handle_selection(event, app_state, sender)
}

pub fn is_hidden_toggle(event: KeyEvent) -> bool {
    event.code == KeyCode::Char('h') && event.modifiers.contains(KeyModifiers::CONTROL)
}

fn handle_delete(event: KeyEvent, app_state: &mut AppState, ui: &mut UiState) -> Option<bool> {
    let is_delete = (event.code == KeyCode::Char('w')
        && event.modifiers.contains(KeyModifiers::CONTROL))
        || event.code == KeyCode::F(8);
    if is_delete {
        let panel = app_state.active_panel_mut();
        let targets = if panel.multi_selected_count() > 0 {
            panel.multi_selected_paths()
        } else {
            panel.get_selected_path().into_iter().collect()
        };
        if !targets.is_empty() {
            ui.delete_paths = Some(targets);
            return Some(true);
        }
        return Some(false);
    }
    None
}

fn handle_file_ops(event: KeyEvent, app_state: &mut AppState, ui: &mut UiState) -> Option<bool> {
    // Shift+F6 must come BEFORE plain F6.
    if event.code == KeyCode::F(6) && event.modifiers.contains(KeyModifiers::SHIFT) {
        let panel = app_state.active_panel_mut();
        let name = panel
            .selected_entry()
            .map(|e| e.name.clone())
            .unwrap_or_default();
        ui.rename_input = Some(Input::new(name));
        return Some(true);
    }
    // Ctrl+Y ("yank") alias for terminals/keyboards where F5 is awkward (macOS media keys).
    if event.code == KeyCode::F(5)
        || (event.code == KeyCode::Char('y') && event.modifiers.contains(KeyModifiers::CONTROL))
    {
        let panel = app_state.active_panel();
        let sources = if panel.multi_selected_count() > 0 {
            panel.multi_selected_paths()
        } else {
            panel.get_selected_path().into_iter().collect()
        };
        if sources.is_empty() {
            return Some(false);
        }
        let dest = Input::new(opposite_pane_path(app_state));
        ui.copy_dest = Some(CopyMoveState { sources, dest });
        return Some(true);
    }
    if event.code == KeyCode::F(6) {
        let panel = app_state.active_panel();
        let sources = if panel.multi_selected_count() > 0 {
            panel.multi_selected_paths()
        } else {
            panel.get_selected_path().into_iter().collect()
        };
        if sources.is_empty() {
            return Some(false);
        }
        let dest = Input::new(opposite_pane_path(app_state));
        ui.move_dest = Some(CopyMoveState { sources, dest });
        return Some(true);
    }
    if event.code == KeyCode::F(7) {
        ui.new_folder_input = Some(Input::default());
        return Some(true);
    }
    None
}

fn opposite_pane_path(app_state: &AppState) -> String {
    if app_state.two_pane_mode {
        use crate::application::ActivePane;
        let other = match app_state.active_pane {
            ActivePane::Left => &app_state.right_panel,
            ActivePane::Right => &app_state.left_panel,
        };
        other.current_path.to_string_lossy().into_owned()
    } else {
        String::new()
    }
}

fn handle_search(event: KeyEvent, ui: &mut UiState) -> Option<bool> {
    if event.code == KeyCode::Char('!')
        && !event
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        ui.shell_command = Some(Input::default());
        return Some(true);
    }
    if event.code == KeyCode::Char('f') && event.modifiers.contains(KeyModifiers::CONTROL) {
        ui.find_filter = Some(Input::default());
        return Some(true);
    }
    if event.code == KeyCode::Char('r') && event.modifiers.contains(KeyModifiers::CONTROL) {
        ui.ripgrep_filter = Some(Input::default());
        return Some(true);
    }
    None
}

fn handle_favorites(event: KeyEvent, app_state: &mut AppState, ui: &mut UiState) -> Option<bool> {
    if event.code == KeyCode::Char('d') && event.modifiers.contains(KeyModifiers::CONTROL) {
        let panel = app_state.active_panel_mut();
        if event.modifiers.contains(KeyModifiers::SHIFT) {
            add_favorite(&mut ui.favorites_items, &panel.current_path);
        }
        ui.favorites_active = true;
        return Some(true);
    }
    None
}

fn handle_two_pane_toggle(event: KeyEvent, app_state: &mut AppState) -> Option<bool> {
    if event.code == KeyCode::Char('t') && event.modifiers.contains(KeyModifiers::CONTROL) {
        app_state.two_pane_mode = !app_state.two_pane_mode;
        return Some(true);
    }
    None
}

fn handle_vscode(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    vscode_adapter: &VsCodeAdapter,
) -> Option<bool> {
    if event.code == KeyCode::F(4)
        || (event.code == KeyCode::Char('o') && event.modifiers.contains(KeyModifiers::CONTROL))
    {
        let panel = app_state.active_panel_mut();
        if let Some(entry) = panel.selected_entry() {
            let path = entry.path.clone();
            let line = panel.search_match_lines.get(&path).copied();
            // Terminal editors need the TTY to themselves: suspend, block, resume.
            if let Some(parts) = vscode_adapter.terminal_editor() {
                renderer.suspend(|| {
                    if let Some(line) = line {
                        vscode_adapter.open_in_terminal_at_line(&parts, &path, line)
                    } else {
                        vscode_adapter.open_in_terminal(&parts, &path)
                    }
                });
                return Some(true);
            }
            if let Some(line) = line {
                vscode_adapter.open_at_line(&path, line);
            } else {
                vscode_adapter.open(&path);
            }
        }
        return Some(false);
    }
    None
}

fn handle_selection(
    event: KeyEvent,
    app_state: &mut AppState,
    sender: &Sender<Message>,
) -> Option<bool> {
    let panel = app_state.active_panel_mut();

    // Multi-select (Space)
    if event.code == KeyCode::Char(' ') && !matches!(panel.mode, PanelMode::QuickView(_)) {
        let before = panel.multi_selected_count();
        panel.toggle_multi_selection(panel.cursor);
        let changed = panel.multi_selected_count() != before;
        if changed {
            schedule_size_jobs(app_state, sender);
        }
        return Some(changed);
    }

    let panel = app_state.active_panel_mut();
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
