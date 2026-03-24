//! Event loop - main application loop.

use crate::application::use_cases::navigate;
use crate::application::{ActivePane, AppState, PanelMode};
use crate::infrastructure::{StdFileSystem, SystemClipboard, SystemOpenAdapter, VsCodeAdapter};
use crate::presentation::TerminalRenderer;
use crate::presentation::input_handler::{
    Message, handle_delete_confirmation, handle_favorites_input, handle_filter_mode,
    handle_find_input, handle_normal_mode, handle_quick_view_mode, handle_ripgrep_input, is_quit,
};
use crate::presentation::shortcuts;
use crate::presentation::terminal::overlays;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::path::PathBuf;
use std::process::exit;
use std::sync::mpsc::{Receiver, Sender};

#[allow(clippy::too_many_arguments)]
pub fn run(
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    fs_adapter: &StdFileSystem,
    open_adapter: &SystemOpenAdapter,
    vscode_adapter: &VsCodeAdapter,
    clipboard_adapter: &mut SystemClipboard,
    sender: &Sender<Message>,
    receiver: &Receiver<Message>,
    delete_paths: &mut Option<Vec<PathBuf>>,
    find_filter: &mut Option<String>,
    ripgrep_filter: &mut Option<String>,
    favorites_active: &mut bool,
    favorites_items: &mut Vec<String>,
    favorites_selected: &mut usize,
    two_pane: bool,
) {
    loop {
        if event::poll(std::time::Duration::from_millis(100)).unwrap() {
            match event::read().unwrap() {
                Event::Key(ev) if ev.kind == event::KeyEventKind::Press => {
                    if is_quit(ev) {
                        renderer.reset_terminal();
                        exit(0);
                    }
                    route_input(
                        ev,
                        app_state,
                        renderer,
                        fs_adapter,
                        open_adapter,
                        vscode_adapter,
                        clipboard_adapter,
                        sender,
                        delete_paths,
                        find_filter,
                        ripgrep_filter,
                        favorites_active,
                        favorites_items,
                        favorites_selected,
                        two_pane,
                    );
                }
                Event::Resize(cols, rows) => {
                    renderer.update_size(cols, rows);
                    overlays::draw(renderer, app_state);
                }
                _ => {}
            }
        }
        handle_async_messages(receiver, app_state, renderer);
        app_state
            .active_panel_mut()
            .clear_notification_if_expired(3000);
    }
}

#[allow(clippy::too_many_arguments)]
fn route_input(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    fs_adapter: &StdFileSystem,
    open_adapter: &SystemOpenAdapter,
    vscode_adapter: &VsCodeAdapter,
    clipboard_adapter: &mut SystemClipboard,
    sender: &Sender<Message>,
    delete_paths: &mut Option<Vec<PathBuf>>,
    find_filter: &mut Option<String>,
    ripgrep_filter: &mut Option<String>,
    favorites_active: &mut bool,
    favorites_items: &mut Vec<String>,
    favorites_selected: &mut usize,
    two_pane: bool,
) {
    if handle_help(event, app_state, renderer) {
        return;
    }
    if handle_modal_overlays(
        event,
        app_state,
        renderer,
        fs_adapter,
        sender,
        delete_paths,
        find_filter,
        ripgrep_filter,
        favorites_active,
        favorites_items,
        favorites_selected,
    ) {
        return;
    }
    if two_pane && event.code == KeyCode::Tab {
        app_state.switch_pane();
        overlays::draw(renderer, app_state);
        return;
    }
    if shortcuts::handle(
        event,
        app_state,
        renderer,
        vscode_adapter,
        delete_paths,
        find_filter,
        ripgrep_filter,
        favorites_active,
        favorites_items,
        favorites_selected,
    ) {
        return;
    }
    handle_mode_input(
        event,
        app_state,
        renderer,
        fs_adapter,
        open_adapter,
        clipboard_adapter,
        sender,
    );
    overlays::draw(renderer, app_state);
}

fn handle_help(event: KeyEvent, app_state: &mut AppState, renderer: &mut TerminalRenderer) -> bool {
    if app_state.show_help {
        if matches!(event.code, KeyCode::F(1) | KeyCode::Esc) {
            app_state.show_help = false;
        }
        overlays::draw(renderer, app_state);
        return true;
    }
    if event.code == KeyCode::F(1) {
        app_state.show_help = !app_state.show_help;
        overlays::draw(renderer, app_state);
        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn handle_modal_overlays(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    fs_adapter: &StdFileSystem,
    sender: &Sender<Message>,
    delete_paths: &mut Option<Vec<PathBuf>>,
    find_filter: &mut Option<String>,
    ripgrep_filter: &mut Option<String>,
    favorites_active: &mut bool,
    favorites_items: &[String],
    favorites_selected: &mut usize,
) -> bool {
    if *favorites_active {
        handle_favorites_input(
            event,
            favorites_active,
            favorites_items,
            favorites_selected,
            fs_adapter,
            app_state,
        );
        overlays::draw_with_favorites(
            renderer,
            app_state,
            *favorites_active,
            favorites_items,
            *favorites_selected,
        );
        return true;
    }
    if delete_paths.is_some() {
        handle_delete_confirmation(event, delete_paths, fs_adapter, app_state);
        overlays::draw_with_delete(renderer, app_state, delete_paths.as_ref());
        return true;
    }
    if find_filter.is_some() {
        handle_find_input(event, find_filter, sender, fs_adapter, app_state);
        overlays::draw_with_find(renderer, app_state, find_filter.as_ref());
        return true;
    }
    if ripgrep_filter.is_some() {
        handle_ripgrep_input(event, ripgrep_filter, fs_adapter, app_state);
        overlays::draw_with_ripgrep(renderer, app_state, ripgrep_filter.as_ref());
        return true;
    }
    false
}

fn handle_mode_input(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    fs_adapter: &StdFileSystem,
    open_adapter: &SystemOpenAdapter,
    clipboard_adapter: &mut SystemClipboard,
    sender: &Sender<Message>,
) {
    let panel = app_state.active_panel_mut();
    match &panel.mode {
        PanelMode::Normal => handle_normal_mode(
            event,
            fs_adapter,
            open_adapter,
            clipboard_adapter,
            app_state,
            renderer,
            sender,
        ),
        PanelMode::Filter => handle_filter_mode(
            event,
            fs_adapter,
            open_adapter,
            clipboard_adapter,
            app_state,
            renderer,
            sender,
        ),
        PanelMode::QuickView(_) => handle_quick_view_mode(event, app_state, renderer),
    }
}

fn handle_async_messages(
    receiver: &Receiver<Message>,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
) {
    if let Ok(message) = receiver.try_recv() {
        match message {
            Message::DrawFiles {
                pane,
                base_path,
                files,
            } => {
                let panel = match pane {
                    ActivePane::Left => &mut app_state.left_panel,
                    ActivePane::Right => &mut app_state.right_panel,
                };
                panel.clear_notification_force();
                navigate::replace_entries_from_search(panel, files, &base_path);
                overlays::draw(renderer, app_state);
            }
            Message::QuickViewResult { pane, mode } => {
                let panel = match pane {
                    ActivePane::Left => &mut app_state.left_panel,
                    ActivePane::Right => &mut app_state.right_panel,
                };
                panel.mode = PanelMode::QuickView(mode);
                overlays::draw(renderer, app_state);
            }
        }
    }
}
