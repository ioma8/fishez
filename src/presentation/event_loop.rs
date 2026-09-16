//! Event loop - main application loop.

use crate::application::ports::ClipboardPort;
use crate::application::use_cases::navigate;
use crate::application::use_cases::transfer::{TransferEvent, TransferKind};
use crate::application::{ActivePane, AppState, PanelMode, PanelState, QuickViewMode, SizeFigure};
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
    ui: &mut UiState,
) {
    let mut last_anim_tick: Option<Instant> = None;
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
        schedule_disk_usage(app_state, sender);
        handle_async_messages(receiver, app_state, renderer, fs_adapter, ui);
        if advance_quick_view_animation(app_state, renderer, &mut last_anim_tick) {
            continue;
        }
        if clear_expired_notification(app_state, 3000) {
            redraw_current_view(renderer, app_state, ui);
        }
    }
}

/// Steps the active panel's animated quick view forward once its current frame
/// delay has elapsed, then redraws. `last_tick` is the baseline of the current
/// frame; it resets whenever the mode isn't an animation (e.g. between previews).
fn advance_quick_view_animation(
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    last_tick: &mut Option<Instant>,
) -> bool {
    let delay_ms = {
        let panel = app_state.active_panel();
        let PanelMode::QuickView(QuickViewMode::Animated { delays, frame, .. }) = &panel.mode
        else {
            *last_tick = None;
            return false;
        };
        delays[*frame]
    };
    let due = match last_tick {
        // First sighting of this animation: start its clock without advancing,
        // so the first frame is shown for its full delay.
        None => {
            *last_tick = Some(Instant::now());
            return false;
        }
        Some(t) => t.elapsed() >= Duration::from_millis(delay_ms),
    };
    if !due {
        return false;
    }
    let panel = app_state.active_panel_mut();
    let PanelMode::QuickView(QuickViewMode::Animated { delays, frame, .. }) = &mut panel.mode
    else {
        return false;
    };
    *frame = (*frame + 1) % delays.len();
    *last_tick = Some(Instant::now());
    overlays::draw(renderer, app_state);
    true
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
            sender,
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
            sender,
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

fn schedule_disk_usage(state: &mut AppState, sender: &Sender<Message>) {
    let pane = state.active_pane;
    let panel = state.active_panel_mut();
    if panel.disk_usage_pending {
        return;
    }
    if panel.disk_usage_path == panel.current_path
        && panel
            .disk_usage_checked
            .is_some_and(|time| time.elapsed() < Duration::from_secs(5))
    {
        return;
    }
    if panel.disk_usage_path != panel.current_path {
        panel.disk_usage = None;
    }
    panel.disk_usage_path = panel.current_path.clone();
    panel.disk_usage_pending = true;
    panel.disk_usage_checked = Some(Instant::now());
    let path = panel.current_path.clone();
    let sender = sender.clone();
    std::thread::spawn(move || {
        let usage = crate::infrastructure::disk_free_and_total(&path);
        let _ = sender.send(Message::DiskUsage { pane, path, usage });
    });
}

fn handle_async_messages(
    receiver: &Receiver<Message>,
    app_state: &mut AppState,
    renderer: &mut TerminalRenderer,
    fs_adapter: &StdFileSystem,
    ui: &mut UiState,
) {
    let mut dirty = false;
    for message in receiver.try_iter() {
        match message {
            Message::DiskUsage { pane, path, usage } => {
                let panel = pane_panel_mut(app_state, pane);
                panel.disk_usage_pending = false;
                if panel.current_path == path && panel.disk_usage != usage {
                    panel.disk_usage = usage;
                    dirty = true;
                }
            }
            Message::DeleteDone { pane, path, result } => {
                let panel = pane_panel_mut(app_state, pane);
                if panel.current_path == path {
                    navigate::refresh_entries(fs_adapter, panel);
                }
                panel.set_notification(match result {
                    Ok(()) => "Deleted".into(),
                    Err(error) => error,
                });
                dirty = true;
            }
            Message::DrawFiles {
                pane,
                base_path,
                files,
                generation,
            } => {
                let panel = pane_panel_mut(app_state, pane);
                if panel.current_path != base_path || panel.search_generation != generation {
                    continue;
                }
                panel.clear_notification_force();
                navigate::replace_entries_from_search(panel, files, &base_path);
                dirty = true;
            }
            Message::QuickViewResult {
                pane,
                generation,
                mode,
            } => {
                let panel = pane_panel_mut(app_state, pane);
                if apply_quick_view_result(panel, generation, mode) {
                    dirty = true;
                }
            }
            Message::Transfer(event) => {
                handle_transfer_event(event, app_state, fs_adapter, ui);
                dirty = true;
            }
            Message::DirTotalResult {
                pane,
                generation,
                total,
            } => {
                let panel = pane_panel_mut(app_state, pane);
                if apply_dir_total_result(panel, generation, total) {
                    dirty = true;
                }
            }
            Message::SelectionTotalResult {
                pane,
                generation,
                total,
            } => {
                let panel = pane_panel_mut(app_state, pane);
                if apply_selection_total_result(panel, generation, total) {
                    dirty = true;
                }
            }
        }
    }
    if dirty {
        redraw_current_view(renderer, app_state, ui);
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
            }
        }
        TransferEvent::Conflict { path, reply } => {
            if let Some(job) = ui.transfer_state.as_mut() {
                job.pending_conflict =
                    Some(crate::presentation::input_handler::PendingConflict { path, reply });
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_async_results_redraw_once_and_ignore_stale_search() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut state = AppState::new(false);
        state.show_onboarding = false;
        let path = state.left_panel.current_path.clone();
        for generation in [0, 1] {
            tx.send(Message::DrawFiles {
                pane: ActivePane::Left,
                base_path: path.clone(),
                files: vec![format!("result-{generation}")],
                generation,
            })
            .unwrap();
        }
        tx.send(Message::DiskUsage {
            pane: ActivePane::Left,
            path,
            usage: Some((1024, 4096)),
        })
        .unwrap();
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(80, 24);
        handle_async_messages(
            &rx,
            &mut state,
            &mut renderer,
            &StdFileSystem,
            &mut UiState::default(),
        );
        assert_eq!(state.left_panel.entries[0].name, "result-0");
        let output = String::from_utf8(buffer.borrow().clone()).unwrap();
        assert_eq!(output.matches("PWD:").count(), 1);
    }

    #[test]
    fn test_delete_completion_clears_selection() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut state = AppState::new(false);
        state.left_panel.multi_selected.insert(0);
        tx.send(Message::DeleteDone {
            pane: ActivePane::Left,
            path: state.left_panel.current_path.clone(),
            result: Ok(()),
        })
        .unwrap();
        let (mut renderer, _) = TerminalRenderer::with_test_writer(80, 24);
        handle_async_messages(
            &rx,
            &mut state,
            &mut renderer,
            &StdFileSystem,
            &mut UiState::default(),
        );
        assert_eq!(state.left_panel.multi_selected_count(), 0);
        assert_eq!(state.left_panel.notification.as_deref(), Some("Deleted"));
    }

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
    fn animation_first_tick_sets_baseline_without_advancing() {
        let mut state = AppState::new(false);
        state.active_panel_mut().mode = PanelMode::QuickView(QuickViewMode::Animated {
            frames: std::sync::Arc::new(vec![vec![1], vec![2]]),
            delays: vec![10, 10],
            frame: 0,
        });
        let (mut renderer, _buffer) = TerminalRenderer::with_test_writer(80, 24);
        let mut last_tick = None;

        assert!(!advance_quick_view_animation(
            &mut state,
            &mut renderer,
            &mut last_tick
        ));
        assert!(last_tick.is_some());
        assert_eq!(
            anim_frame(&state),
            0,
            "first sighting must not advance past frame 0"
        );
    }

    #[test]
    fn animation_advances_after_elapsed_delay_and_loops() {
        let mut state = AppState::new(false);
        state.active_panel_mut().mode = PanelMode::QuickView(QuickViewMode::Animated {
            frames: std::sync::Arc::new(vec![vec![1], vec![2]]),
            delays: vec![10, 10],
            frame: 0,
        });
        let (mut renderer, _buffer) = TerminalRenderer::with_test_writer(80, 24);
        let mut last_tick = Some(Instant::now());

        // not due yet: stays on frame 0
        assert!(!advance_quick_view_animation(
            &mut state,
            &mut renderer,
            &mut last_tick
        ));
        assert_eq!(anim_frame(&state), 0);

        // a stale tick is due: advances to frame 1
        last_tick = Some(Instant::now() - Duration::from_secs(1));
        assert!(advance_quick_view_animation(
            &mut state,
            &mut renderer,
            &mut last_tick
        ));
        assert_eq!(anim_frame(&state), 1);

        // and loops back to frame 0
        last_tick = Some(Instant::now() - Duration::from_secs(1));
        assert!(advance_quick_view_animation(
            &mut state,
            &mut renderer,
            &mut last_tick
        ));
        assert_eq!(anim_frame(&state), 0);
    }

    #[test]
    fn non_animation_mode_resets_the_animation_tick() {
        let mut state = AppState::new(false);
        let (mut renderer, _buffer) = TerminalRenderer::with_test_writer(80, 24);
        let mut last_tick = Some(Instant::now());

        assert!(!advance_quick_view_animation(
            &mut state,
            &mut renderer,
            &mut last_tick
        ));
        assert_eq!(last_tick, None, "non-animation mode must reset the timer");
    }

    fn anim_frame(state: &AppState) -> usize {
        let PanelMode::QuickView(QuickViewMode::Animated { frame, .. }) =
            &state.active_panel().mode
        else {
            panic!("expected Animated mode");
        };
        *frame
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
