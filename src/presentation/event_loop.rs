//! Event loop - main application loop.

use crate::application::ports::ClipboardPort;
use crate::application::use_cases::navigate;
use crate::application::use_cases::transfer::{TransferEvent, TransferKind};
use crate::application::{ActivePane, AppState, PanelMode, PanelState, SizeFigure};
use crate::infrastructure::{
    StdFileSystem, SystemClipboard, SystemOpenAdapter, VsCodeAdapter, mark_onboarded,
};
use crate::presentation::TerminalRenderer;
use crate::presentation::input_handler::{
    ContextMenuAction, ContextMenuResponse, Message, MouseOutcome, OverlayKind, UiState,
    handle_context_menu_input, handle_copy_move_dest_input, handle_delete_confirmation,
    handle_favorites_input, handle_filter_mode, handle_find_input, handle_mouse_event,
    handle_new_folder_input, handle_normal_mode, handle_quick_view_mode, handle_rename_input,
    handle_ripgrep_input, handle_shell_input, handle_transfer_input, is_quit, schedule_quick_view,
};
use crate::presentation::shortcuts;
use crate::presentation::terminal::overlays;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
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
    ui: &mut UiState,
) {
    loop {
        if event::poll(std::time::Duration::from_millis(15)).unwrap() {
            match event::read().unwrap() {
                Event::Key(ev) if ev.kind == event::KeyEventKind::Press => {
                    if dismiss_onboarding(app_state) {
                        mark_onboarded();
                        redraw_current_view(renderer, app_state, ui);
                    }
                    if is_quit(ev) || should_quit_with_escape(ev, app_state, ui) {
                        renderer.reset_terminal();
                        return;
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
                        ui,
                    );
                }
                Event::Mouse(ev) => {
                    let outcome = handle_mouse_event(
                        ev,
                        app_state,
                        renderer.rows,
                        renderer.columns,
                        &mut ui.context_menu,
                    );
                    match outcome {
                        MouseOutcome::Nothing => {}
                        MouseOutcome::Redraw => {
                            redraw_current_view(renderer, app_state, ui);
                        }
                        MouseOutcome::ExecuteContext {
                            action,
                            target,
                            name,
                        } => {
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
                                renderer,
                                ui,
                            );
                            redraw_current_view(renderer, app_state, ui);
                        }
                    }
                }
                Event::Resize(cols, rows) => {
                    renderer.update_size(cols, rows);
                    redraw_current_view(renderer, app_state, ui);
                }
                _ => {}
            }
        }
        handle_async_messages(receiver, app_state, renderer, fs_adapter, ui);
        if clear_expired_notification(app_state, 3000) {
            redraw_current_view(renderer, app_state, ui);
        }
    }
}

fn dismiss_onboarding(app_state: &mut AppState) -> bool {
    if !app_state.show_onboarding {
        return false;
    }
    app_state.show_onboarding = false;
    true
}

/// Esc quits only from a clean state: no overlay, no multi-selection, normal mode.
fn should_quit_with_escape(event: KeyEvent, app_state: &AppState, ui: &UiState) -> bool {
    event.code == KeyCode::Esc
        && !app_state.show_help
        && !ui.has_active_overlay()
        && app_state.active_panel().multi_selected_count() == 0
        && matches!(app_state.active_panel().mode, PanelMode::Normal)
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
    ui: &mut UiState,
) {
    // Context menu takes absolute priority when open.
    if ui.context_menu.is_some() {
        match handle_context_menu_input(event, &mut ui.context_menu) {
            ContextMenuResponse::Handled | ContextMenuResponse::Close => {}
            ContextMenuResponse::Execute {
                action,
                target,
                name,
            } => {
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
                    renderer,
                    ui,
                );
            }
        }
        redraw_current_view(renderer, app_state, ui);
        return;
    }

    if let Some(needs_redraw) = handle_help(event, app_state) {
        if needs_redraw {
            redraw_current_view(renderer, app_state, ui);
        }
        return;
    }
    if let Some(needs_redraw) = handle_modal_overlays(event, app_state, fs_adapter, sender, ui) {
        if needs_redraw {
            redraw_current_view(renderer, app_state, ui);
        }
        return;
    }
    if app_state.two_pane_mode && event.code == KeyCode::Tab {
        app_state.switch_pane();
        redraw_current_view(renderer, app_state, ui);
        return;
    }
    if shortcuts::is_hidden_toggle(event) {
        let show_hidden = !app_state.left_panel.show_hidden;
        app_state.left_panel.show_hidden = show_hidden;
        app_state.right_panel.show_hidden = show_hidden;
        navigate::refresh_entries(fs_adapter, &mut app_state.left_panel);
        navigate::refresh_entries(fs_adapter, &mut app_state.right_panel);
        redraw_current_view(renderer, app_state, ui);
        return;
    }
    if let Some(needs_redraw) =
        shortcuts::handle(event, app_state, renderer, vscode_adapter, ui, sender)
    {
        if needs_redraw {
            redraw_current_view(renderer, app_state, ui);
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
        redraw_current_view(renderer, app_state, ui);
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_context_action(
    action: ContextMenuAction,
    target: std::path::PathBuf,
    name: String,
    app_state: &mut AppState,
    fs_adapter: &StdFileSystem,
    open_adapter: &SystemOpenAdapter,
    vscode_adapter: &VsCodeAdapter,
    clipboard_adapter: &mut SystemClipboard,
    sender: &Sender<Message>,
    renderer: &mut TerminalRenderer,
    ui: &mut UiState,
) {
    let renderer_columns = renderer.columns;
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
            ui.rename_input = Some(tui_input::Input::new(name));
        }
        ContextMenuAction::Delete => {
            ui.delete_paths = Some(vec![target]);
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
            if let Some(parts) = vscode_adapter.terminal_editor() {
                renderer.suspend(|| vscode_adapter.open_in_terminal(&parts, &target));
            } else {
                vscode_adapter.open(&target);
            }
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

/// Routes input to whichever overlay currently owns it, in `OverlayKind` priority order.
/// The context menu is handled separately in `route_input`, before this is reached.
fn handle_modal_overlays(
    event: KeyEvent,
    app_state: &mut AppState,
    fs_adapter: &StdFileSystem,
    sender: &Sender<Message>,
    ui: &mut UiState,
) -> Option<bool> {
    match ui.active_overlay() {
        Some(OverlayKind::Transfer) => Some(handle_transfer_input(event, &mut ui.transfer_state)),
        Some(OverlayKind::Favorites) => Some(handle_favorites_input(
            event,
            &mut ui.favorites_active,
            &ui.favorites_items,
            &mut ui.favorites_selected,
            fs_adapter,
            app_state,
        )),
        Some(OverlayKind::Delete) => Some(handle_delete_confirmation(
            event,
            &mut ui.delete_paths,
            fs_adapter,
            app_state,
        )),
        Some(OverlayKind::Find) => Some(handle_find_input(
            event,
            &mut ui.find_filter,
            sender,
            fs_adapter,
            app_state,
        )),
        Some(OverlayKind::Ripgrep) => Some(handle_ripgrep_input(
            event,
            &mut ui.ripgrep_filter,
            fs_adapter,
            app_state,
        )),
        Some(OverlayKind::Shell) => Some(handle_shell_input(
            event,
            &mut ui.shell_command,
            &mut ui.shell_history,
            &mut ui.shell_history_idx,
            sender,
            app_state,
        )),
        Some(OverlayKind::Rename) => Some(handle_rename_input(
            event,
            &mut ui.rename_input,
            fs_adapter,
            app_state,
        )),
        Some(OverlayKind::NewFolder) => Some(handle_new_folder_input(
            event,
            &mut ui.new_folder_input,
            fs_adapter,
            app_state,
        )),
        Some(OverlayKind::CopyDest) => Some(handle_copy_move_dest_input(
            event,
            &mut ui.copy_dest,
            &mut ui.transfer_state,
            sender,
            app_state,
            TransferKind::Copy,
            false,
        )),
        Some(OverlayKind::MoveDest) => Some(handle_copy_move_dest_input(
            event,
            &mut ui.move_dest,
            &mut ui.transfer_state,
            sender,
            app_state,
            TransferKind::Move,
            true,
        )),
        Some(OverlayKind::ContextMenu) | None => None,
    }
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
        PanelMode::QuickView(_) => handle_quick_view_mode(event, app_state, renderer, sender),
    }
}

/// Redraws the base view plus whichever overlay is active (same priority order
/// as `handle_modal_overlays`), then floats the context menu on top.
fn redraw_current_view(renderer: &mut TerminalRenderer, app_state: &AppState, ui: &UiState) {
    match ui.active_overlay() {
        Some(OverlayKind::Favorites) => {
            overlays::draw_favorites_overlay(renderer, &ui.favorites_items, ui.favorites_selected)
        }
        Some(OverlayKind::Delete) => {
            overlays::draw_with_delete(renderer, app_state, ui.delete_paths.as_ref())
        }
        Some(OverlayKind::Find) => {
            overlays::draw_with_prompt(renderer, app_state, "Find", ui.find_filter.as_ref())
        }
        Some(OverlayKind::Ripgrep) => {
            overlays::draw_with_prompt(renderer, app_state, "RipGrep", ui.ripgrep_filter.as_ref())
        }
        Some(OverlayKind::Shell) => {
            overlays::draw_with_prompt(renderer, app_state, "!", ui.shell_command.as_ref())
        }
        Some(OverlayKind::Rename) => {
            overlays::draw_with_prompt(renderer, app_state, "Rename", ui.rename_input.as_ref())
        }
        Some(OverlayKind::NewFolder) => overlays::draw_with_prompt(
            renderer,
            app_state,
            "New folder",
            ui.new_folder_input.as_ref(),
        ),
        Some(OverlayKind::CopyDest) => {
            if let Some(state) = ui.copy_dest.as_ref() {
                overlays::draw_with_copy_dest(renderer, app_state, &state.dest);
            }
        }
        Some(OverlayKind::MoveDest) => {
            if let Some(state) = ui.move_dest.as_ref() {
                overlays::draw_with_move_dest(renderer, app_state, &state.dest);
            }
        }
        Some(OverlayKind::Transfer) => {
            if let Some(job) = ui.transfer_state.as_ref() {
                overlays::draw_with_transfer(renderer, app_state, job);
            }
        }
        Some(OverlayKind::ContextMenu) | None => overlays::draw(renderer, app_state),
    }
    // Context menu floats on top of whatever was already drawn.
    if let Some(ctx) = ui.context_menu.as_ref() {
        overlays::draw_context_menu(renderer, ctx);
    }
}

fn clear_expired_notification(app_state: &mut AppState, timeout_ms: u64) -> bool {
    let panel = app_state.active_panel_mut();
    let had_notification = panel.notification.is_some();
    panel.clear_notification_if_expired(timeout_ms);
    had_notification && panel.notification.is_none()
}

fn pane_panel_mut(app_state: &mut AppState, pane: ActivePane) -> &mut PanelState {
    match pane {
        ActivePane::Left => &mut app_state.left_panel,
        ActivePane::Right => &mut app_state.right_panel,
    }
}

fn handle_async_messages(
    receiver: &Receiver<Message>,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    fs_adapter: &StdFileSystem,
    ui: &mut UiState,
) {
    while let Ok(message) = receiver.try_recv() {
        match message {
            Message::DrawFiles {
                pane,
                base_path,
                files,
            } => {
                let panel = pane_panel_mut(app_state, pane);
                panel.clear_notification_force();
                navigate::replace_entries_from_search(panel, files, &base_path);
                overlays::draw(renderer, app_state);
            }
            Message::QuickViewResult {
                pane,
                generation,
                mode,
            } => {
                let panel = pane_panel_mut(app_state, pane);
                if apply_quick_view_result(panel, generation, mode) {
                    overlays::draw(renderer, app_state);
                }
            }
            Message::Transfer(event) => {
                handle_transfer_event(event, app_state, renderer, fs_adapter, ui);
            }
            Message::DirTotalResult {
                pane,
                generation,
                total,
            } => {
                let panel = pane_panel_mut(app_state, pane);
                if apply_dir_total_result(panel, generation, total) {
                    overlays::draw(renderer, app_state);
                }
            }
            Message::SelectionTotalResult {
                pane,
                generation,
                total,
            } => {
                let panel = pane_panel_mut(app_state, pane);
                if apply_selection_total_result(panel, generation, total) {
                    overlays::draw(renderer, app_state);
                }
            }
        }
    }
}

fn apply_quick_view_result(
    panel: &mut PanelState,
    generation: u64,
    mode: crate::application::QuickViewMode,
) -> bool {
    if panel.quick_view_generation == generation {
        panel.mode = PanelMode::QuickView(mode);
        true
    } else {
        false
    }
}

/// Applies a directory-total background result, discarding it if a newer generation
/// (e.g. navigating to another directory) has superseded it. Returns whether it was applied.
fn apply_dir_total_result(panel: &mut PanelState, generation: u64, total: u64) -> bool {
    if panel.dir_total_generation == generation {
        panel.dir_total = SizeFigure::Ready(total);
        panel
            .dir_total_cache
            .insert(panel.current_path.clone(), total);
        true
    } else {
        false
    }
}

/// Applies a selection-total background result, discarding it if a newer generation
/// (e.g. the selection changed again) has superseded it. Returns whether it was applied.
fn apply_selection_total_result(panel: &mut PanelState, generation: u64, total: u64) -> bool {
    if panel.selection_total_generation == generation {
        panel.selection_total = SizeFigure::Ready(total);
        true
    } else {
        false
    }
}

fn handle_transfer_event(
    event: TransferEvent,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    fs_adapter: &StdFileSystem,
    ui: &mut UiState,
) {
    match event {
        TransferEvent::Progress {
            current,
            done,
            total,
        } => {
            if let Some(job) = ui.transfer_state.as_mut() {
                job.current = current;
                job.done = done;
                job.total = total;
                overlays::draw_with_transfer(renderer, app_state, job);
            }
        }
        TransferEvent::Conflict { path, reply } => {
            if let Some(job) = ui.transfer_state.as_mut() {
                job.pending_conflict =
                    Some(crate::presentation::input_handler::PendingConflict { path, reply });
                overlays::draw_with_transfer(renderer, app_state, job);
            }
        }
        TransferEvent::Done(outcome) => {
            let crate::application::use_cases::transfer::Outcome {
                ok,
                failed,
                cancelled,
            } = outcome;
            navigate::refresh_entries(fs_adapter, &mut app_state.left_panel);
            navigate::refresh_entries(fs_adapter, &mut app_state.right_panel);
            let verb = match ui.transfer_state.as_ref().map(|j| j.kind) {
                Some(TransferKind::Move) => "Moved",
                _ => "Copied",
            };
            let summary = match (cancelled, failed.len()) {
                (true, 0) => format!("{} cancelled after {} item(s)", verb, ok),
                (true, n) => format!("{} cancelled after {} item(s), {} failed", verb, ok, n),
                (false, 0) => format!("{} {} item(s)", verb, ok),
                (false, n) => format!("{} {} item(s), {} failed", verb, ok, n),
            };
            ui.transfer_state = None;
            app_state.active_panel_mut().set_notification(summary);
            overlays::draw(renderer, app_state);
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

    #[test]
    fn dismiss_onboarding_only_reports_when_visible() {
        let mut state = AppState::new(false);
        state.show_onboarding = true;

        assert!(dismiss_onboarding(&mut state));
        assert!(!state.show_onboarding);
        assert!(!dismiss_onboarding(&mut state));
    }

    #[test]
    fn dir_total_result_applies_when_generation_matches() {
        let mut panel = PanelState::new();
        panel.dir_total_generation = 3;
        assert!(apply_dir_total_result(&mut panel, 3, 12345));
        assert_eq!(panel.dir_total, SizeFigure::Ready(12345));
    }

    #[test]
    fn dir_total_result_populates_the_path_cache() {
        let mut panel = PanelState::new();
        panel.current_path = std::path::PathBuf::from("/some/dir");
        panel.dir_total_generation = 1;
        assert!(apply_dir_total_result(&mut panel, 1, 42));
        assert_eq!(
            panel.dir_total_cache.get(std::path::Path::new("/some/dir")),
            Some(&42)
        );
    }

    #[test]
    fn stale_dir_total_result_is_discarded() {
        let mut panel = PanelState::new();
        panel.dir_total_generation = 3;
        panel.dir_total = SizeFigure::Computing(3);
        // A result for an older generation (e.g. the user navigated away mid-walk).
        assert!(!apply_dir_total_result(&mut panel, 2, 999));
        assert_eq!(panel.dir_total, SizeFigure::Computing(3));
    }

    #[test]
    fn selection_total_result_applies_when_generation_matches() {
        let mut panel = PanelState::new();
        panel.selection_total_generation = 5;
        assert!(apply_selection_total_result(&mut panel, 5, 4096));
        assert_eq!(panel.selection_total, SizeFigure::Ready(4096));
    }

    #[test]
    fn stale_selection_total_result_is_discarded() {
        let mut panel = PanelState::new();
        panel.selection_total_generation = 5;
        panel.selection_total = SizeFigure::Computing(5);
        // A result for a superseded selection (the user toggled again before this arrived).
        assert!(!apply_selection_total_result(&mut panel, 4, 999));
        assert_eq!(panel.selection_total, SizeFigure::Computing(5));
    }

    #[test]
    fn stale_quick_view_result_is_discarded() {
        let mut panel = PanelState::new();
        panel.quick_view_generation = 2;
        panel.mode = PanelMode::Normal;

        assert!(!apply_quick_view_result(
            &mut panel,
            1,
            crate::application::QuickViewMode::NotSupported
        ));
        assert_eq!(panel.mode, PanelMode::Normal);
    }

    #[test]
    fn esc_quits_from_clean_normal_state() {
        let state = AppState::new(false);
        let ui = UiState::default();

        assert!(should_quit_with_escape(
            KeyEvent::from(KeyCode::Esc),
            &state,
            &ui,
        ));
    }

    #[test]
    fn esc_does_not_quit_while_filtering() {
        let mut state = AppState::new(false);
        state.active_panel_mut().mode = PanelMode::Filter;
        let ui = UiState::default();

        assert!(!should_quit_with_escape(
            KeyEvent::from(KeyCode::Esc),
            &state,
            &ui,
        ));
    }

    #[test]
    fn esc_does_not_quit_while_an_overlay_is_active() {
        let state = AppState::new(false);
        let ui = UiState {
            find_filter: Some(tui_input::Input::default()),
            ..Default::default()
        };

        assert!(!should_quit_with_escape(
            KeyEvent::from(KeyCode::Esc),
            &state,
            &ui,
        ));
    }
}
