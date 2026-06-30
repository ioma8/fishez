//! Event loop - main application loop.

use crate::application::ports::ClipboardPort;
use crate::application::use_cases::navigate;
use crate::application::{ActivePane, AppState, PanelMode};
use crate::infrastructure::{StdFileSystem, SystemClipboard, SystemOpenAdapter, VsCodeAdapter};
use crate::presentation::TerminalRenderer;
use crate::presentation::input_handler::{
    ContextMenuAction, ContextMenuResponse, ContextMenuState, CopyMoveState, Message,
    MouseOutcome, handle_context_menu_input, handle_copy_dest_input, handle_delete_confirmation,
    handle_favorites_input, handle_filter_mode, handle_find_input, handle_mouse_event,
    handle_move_dest_input, handle_new_folder_input, handle_normal_mode, handle_quick_view_mode,
    handle_rename_input, handle_ripgrep_input, handle_shell_input, is_quit, schedule_quick_view,
};
use crate::presentation::shortcuts;
use crate::presentation::terminal::overlays;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::path::PathBuf;
use std::process::exit;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

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
    shell_command: &mut Option<String>,
    shell_history: &mut Vec<String>,
    shell_history_idx: &mut Option<usize>,
    rename_input: &mut Option<String>,
    new_folder_input: &mut Option<String>,
    copy_dest: &mut Option<CopyMoveState>,
    move_dest: &mut Option<CopyMoveState>,
    context_menu: &mut Option<ContextMenuState>,
    favorites_active: &mut bool,
    favorites_items: &mut Vec<String>,
    favorites_selected: &mut usize,
) {
    let onboarding_start = Instant::now();
    loop {
        if app_state.show_onboarding && onboarding_start.elapsed() > Duration::from_secs(2) {
            app_state.show_onboarding = false;
        }
        if event::poll(std::time::Duration::from_millis(100)).unwrap() {
            match event::read().unwrap() {
                Event::Key(ev) if ev.kind == event::KeyEventKind::Press => {
                    app_state.show_onboarding = false;
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
                        shell_command,
                        shell_history,
                        shell_history_idx,
                        rename_input,
                        new_folder_input,
                        copy_dest,
                        move_dest,
                        context_menu,
                        favorites_active,
                        favorites_items,
                        favorites_selected,
                    );
                }
                Event::Mouse(ev) => {
                    let outcome = handle_mouse_event(
                        ev,
                        app_state,
                        renderer.rows,
                        renderer.columns,
                        context_menu,
                        sender,
                    );
                    match outcome {
                        MouseOutcome::Nothing => {}
                        MouseOutcome::Redraw => {
                            redraw_current_view(
                                renderer, app_state, delete_paths, find_filter, ripgrep_filter,
                                shell_command, rename_input, new_folder_input, copy_dest,
                                move_dest, context_menu, *favorites_active, favorites_items,
                                *favorites_selected,
                            );
                        }
                        MouseOutcome::ExecuteContext { action, target, name } => {
                            execute_context_action(
                                action, target, name, app_state, fs_adapter, open_adapter,
                                vscode_adapter, clipboard_adapter, sender, renderer.columns,
                                rename_input, delete_paths,
                            );
                            redraw_current_view(
                                renderer, app_state, delete_paths, find_filter, ripgrep_filter,
                                shell_command, rename_input, new_folder_input, copy_dest,
                                move_dest, context_menu, *favorites_active, favorites_items,
                                *favorites_selected,
                            );
                        }
                    }
                }
                Event::Resize(cols, rows) => {
                    renderer.update_size(cols, rows);
                    redraw_current_view(
                        renderer,
                        app_state,
                        delete_paths,
                        find_filter,
                        ripgrep_filter,
                        shell_command,
                        rename_input,
                        new_folder_input,
                        copy_dest,
                        move_dest,
                        context_menu,
                        *favorites_active,
                        favorites_items,
                        *favorites_selected,
                    );
                }
                _ => {}
            }
        }
        handle_async_messages(receiver, app_state, renderer);
        if clear_expired_notification(app_state, 3000) {
            redraw_current_view(
                renderer,
                app_state,
                delete_paths,
                find_filter,
                ripgrep_filter,
                shell_command,
                rename_input,
                new_folder_input,
                copy_dest,
                move_dest,
                context_menu,
                *favorites_active,
                favorites_items,
                *favorites_selected,
            );
        }
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
    shell_command: &mut Option<String>,
    shell_history: &mut Vec<String>,
    shell_history_idx: &mut Option<usize>,
    rename_input: &mut Option<String>,
    new_folder_input: &mut Option<String>,
    copy_dest: &mut Option<CopyMoveState>,
    move_dest: &mut Option<CopyMoveState>,
    context_menu: &mut Option<ContextMenuState>,
    favorites_active: &mut bool,
    favorites_items: &mut Vec<String>,
    favorites_selected: &mut usize,
) {
    // Context menu takes absolute priority when open.
    if context_menu.is_some() {
        match handle_context_menu_input(event, context_menu) {
            ContextMenuResponse::Handled | ContextMenuResponse::Close => {}
            ContextMenuResponse::Execute { action, target, name } => {
                execute_context_action(
                    action,
                    target,
                    name,
                    app_state,
                    fs_adapter,
                    open_adapter,
                    vscode_adapter,
                    clipboard_adapter,
                    sender,
                    renderer.columns,
                    rename_input,
                    delete_paths,
                );
            }
        }
        redraw_current_view(
            renderer,
            app_state,
            delete_paths,
            find_filter,
            ripgrep_filter,
            shell_command,
            rename_input,
            new_folder_input,
            copy_dest,
            move_dest,
            context_menu,
            *favorites_active,
            favorites_items,
            *favorites_selected,
        );
        return;
    }

    if let Some(needs_redraw) = handle_help(event, app_state) {
        if needs_redraw {
            redraw_current_view(
                renderer,
                app_state,
                delete_paths,
                find_filter,
                ripgrep_filter,
                shell_command,
                rename_input,
                new_folder_input,
                copy_dest,
                move_dest,
                context_menu,
                *favorites_active,
                favorites_items,
                *favorites_selected,
            );
        }
        return;
    }
    if let Some(needs_redraw) = handle_modal_overlays(
        event,
        app_state,
        fs_adapter,
        sender,
        delete_paths,
        find_filter,
        ripgrep_filter,
        shell_command,
        shell_history,
        shell_history_idx,
        rename_input,
        new_folder_input,
        copy_dest,
        move_dest,
        favorites_active,
        favorites_items,
        favorites_selected,
    ) {
        if needs_redraw {
            redraw_current_view(
                renderer,
                app_state,
                delete_paths,
                find_filter,
                ripgrep_filter,
                shell_command,
                rename_input,
                new_folder_input,
                copy_dest,
                move_dest,
                context_menu,
                *favorites_active,
                favorites_items,
                *favorites_selected,
            );
        }
        return;
    }
    if app_state.two_pane_mode && event.code == KeyCode::Tab {
        app_state.switch_pane();
        redraw_current_view(
            renderer,
            app_state,
            delete_paths,
            find_filter,
            ripgrep_filter,
            shell_command,
            rename_input,
            new_folder_input,
            copy_dest,
            move_dest,
            context_menu,
            *favorites_active,
            favorites_items,
            *favorites_selected,
        );
        return;
    }
    if let Some(needs_redraw) = shortcuts::handle(
        event,
        app_state,
        vscode_adapter,
        delete_paths,
        find_filter,
        ripgrep_filter,
        shell_command,
        rename_input,
        new_folder_input,
        copy_dest,
        move_dest,
        favorites_active,
        favorites_items,
        favorites_selected,
    ) {
        if needs_redraw {
            redraw_current_view(
                renderer,
                app_state,
                delete_paths,
                find_filter,
                ripgrep_filter,
                shell_command,
                rename_input,
                new_folder_input,
                copy_dest,
                move_dest,
                context_menu,
                *favorites_active,
                favorites_items,
                *favorites_selected,
            );
        }
        return;
    }
    if handle_mode_input(
        event,
        app_state,
        renderer,
        fs_adapter,
        open_adapter,
        clipboard_adapter,
        sender,
    ) {
        redraw_current_view(
            renderer,
            app_state,
            delete_paths,
            find_filter,
            ripgrep_filter,
            shell_command,
            rename_input,
            new_folder_input,
            copy_dest,
            move_dest,
            context_menu,
            *favorites_active,
            favorites_items,
            *favorites_selected,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_context_action(
    action: ContextMenuAction,
    target: PathBuf,
    name: String,
    app_state: &mut AppState,
    fs_adapter: &StdFileSystem,
    open_adapter: &SystemOpenAdapter,
    vscode_adapter: &VsCodeAdapter,
    clipboard_adapter: &mut SystemClipboard,
    sender: &Sender<Message>,
    renderer_columns: u16,
    rename_input: &mut Option<String>,
    delete_paths: &mut Option<Vec<PathBuf>>,
) {
    match action {
        ContextMenuAction::Open => {
            if target.is_dir() {
                navigate::change_directory(fs_adapter, app_state.active_panel_mut(), target);
            } else {
                open_adapter.open(&target);
            }
        }
        ContextMenuAction::QuickView => {
            let pane = app_state.active_pane;
            let panel = app_state.active_panel_mut();
            schedule_quick_view(panel, pane, renderer_columns, sender);
        }
        ContextMenuAction::Rename => {
            *rename_input = Some(name);
        }
        ContextMenuAction::Delete => {
            *delete_paths = Some(vec![target]);
        }
        ContextMenuAction::CopyPath => {
            let path_str = target.to_string_lossy().to_string();
            if clipboard_adapter.copy(&path_str).is_ok() {
                app_state
                    .active_panel_mut()
                    .set_notification("Path copied".to_string());
            }
        }
        ContextMenuAction::OpenVsCode => {
            vscode_adapter.open(&target);
        }
    }
}

fn handle_help(event: KeyEvent, app_state: &mut AppState) -> Option<bool> {
    if app_state.show_help {
        if matches!(event.code, KeyCode::F(1) | KeyCode::Esc) {
            app_state.show_help = false;
            return Some(true);
        }
        return Some(false);
    }
    if event.code == KeyCode::F(1) {
        app_state.show_help = !app_state.show_help;
        return Some(true);
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn handle_modal_overlays(
    event: KeyEvent,
    app_state: &mut AppState,
    fs_adapter: &StdFileSystem,
    sender: &Sender<Message>,
    delete_paths: &mut Option<Vec<PathBuf>>,
    find_filter: &mut Option<String>,
    ripgrep_filter: &mut Option<String>,
    shell_command: &mut Option<String>,
    shell_history: &mut Vec<String>,
    shell_history_idx: &mut Option<usize>,
    rename_input: &mut Option<String>,
    new_folder_input: &mut Option<String>,
    copy_dest: &mut Option<CopyMoveState>,
    move_dest: &mut Option<CopyMoveState>,
    favorites_active: &mut bool,
    favorites_items: &[String],
    favorites_selected: &mut usize,
) -> Option<bool> {
    if *favorites_active {
        return Some(handle_favorites_input(
            event,
            favorites_active,
            favorites_items,
            favorites_selected,
            fs_adapter,
            app_state,
        ));
    }
    if delete_paths.is_some() {
        return Some(handle_delete_confirmation(
            event,
            delete_paths,
            fs_adapter,
            app_state,
        ));
    }
    if find_filter.is_some() {
        return Some(handle_find_input(
            event,
            find_filter,
            sender,
            fs_adapter,
            app_state,
        ));
    }
    if ripgrep_filter.is_some() {
        return Some(handle_ripgrep_input(
            event,
            ripgrep_filter,
            fs_adapter,
            app_state,
        ));
    }
    if shell_command.is_some() {
        return Some(handle_shell_input(
            event,
            shell_command,
            shell_history,
            shell_history_idx,
            sender,
            app_state,
        ));
    }
    if rename_input.is_some() {
        return Some(handle_rename_input(event, rename_input, fs_adapter, app_state));
    }
    if new_folder_input.is_some() {
        return Some(handle_new_folder_input(event, new_folder_input, fs_adapter, app_state));
    }
    if copy_dest.is_some() {
        return Some(handle_copy_dest_input(event, copy_dest, fs_adapter, app_state));
    }
    if move_dest.is_some() {
        return Some(handle_move_dest_input(event, move_dest, fs_adapter, app_state));
    }
    None
}

fn handle_mode_input(
    event: KeyEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    fs_adapter: &StdFileSystem,
    open_adapter: &SystemOpenAdapter,
    clipboard_adapter: &mut SystemClipboard,
    sender: &Sender<Message>,
) -> bool {
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

#[allow(clippy::too_many_arguments)]
fn redraw_current_view(
    renderer: &mut TerminalRenderer,
    app_state: &AppState,
    delete_paths: &Option<Vec<PathBuf>>,
    find_filter: &Option<String>,
    ripgrep_filter: &Option<String>,
    shell_command: &Option<String>,
    rename_input: &Option<String>,
    new_folder_input: &Option<String>,
    copy_dest: &Option<CopyMoveState>,
    move_dest: &Option<CopyMoveState>,
    context_menu: &Option<ContextMenuState>,
    favorites_active: bool,
    favorites_items: &[String],
    favorites_selected: usize,
) {
    if favorites_active {
        overlays::draw_with_favorites(
            renderer,
            app_state,
            true,
            favorites_items,
            favorites_selected,
        );
    } else if delete_paths.is_some() {
        overlays::draw_with_delete(renderer, app_state, delete_paths.as_ref());
    } else if find_filter.is_some() {
        overlays::draw_with_find(renderer, app_state, find_filter.as_ref());
    } else if ripgrep_filter.is_some() {
        overlays::draw_with_ripgrep(renderer, app_state, ripgrep_filter.as_ref());
    } else if shell_command.is_some() {
        overlays::draw_with_shell(renderer, app_state, shell_command.as_ref());
    } else if rename_input.is_some() {
        overlays::draw_with_rename(renderer, app_state, rename_input.as_deref());
    } else if new_folder_input.is_some() {
        overlays::draw_with_new_folder(renderer, app_state, new_folder_input.as_deref());
    } else if let Some(state) = copy_dest.as_ref() {
        overlays::draw_with_copy_dest(renderer, app_state, &state.dest);
    } else if let Some(state) = move_dest.as_ref() {
        overlays::draw_with_move_dest(renderer, app_state, &state.dest);
    } else {
        overlays::draw(renderer, app_state);
    }
    // Context menu floats on top of whatever was already drawn.
    if let Some(ctx) = context_menu.as_ref() {
        overlays::draw_context_menu(renderer, ctx);
    }
}

fn clear_expired_notification(app_state: &mut AppState, timeout_ms: u64) -> bool {
    let panel = app_state.active_panel_mut();
    let had_notification = panel.notification.is_some();
    panel.clear_notification_if_expired(timeout_ms);
    had_notification && panel.notification.is_none()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    #[test]
    fn help_overlay_ignores_unmapped_keys_without_redraw() {
        let mut state = AppState::new(false);
        state.show_help = true;

        assert_eq!(
            handle_help(KeyEvent::from(KeyCode::Char('x')), &mut state),
            Some(false)
        );
    }

    #[test]
    fn help_toggle_requests_redraw() {
        let mut state = AppState::new(false);

        assert_eq!(
            handle_help(KeyEvent::from(KeyCode::F(1)), &mut state),
            Some(true)
        );
    }

    #[test]
    fn clearing_expired_notification_marks_dirty() {
        let mut state = AppState::new(false);
        state
            .active_panel_mut()
            .set_notification("Test".to_string());
        std::thread::sleep(std::time::Duration::from_millis(10));

        assert!(clear_expired_notification(&mut state, 1));
    }
}
