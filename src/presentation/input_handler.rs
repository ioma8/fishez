//! Input handler - keyboard event processing.

use crate::application::ports::FileSystemPort;
use crate::application::use_cases::transfer::{self, ConflictResolution, TransferKind};
use crate::application::use_cases::{file_ops, navigate, new_folder, quick_view};
use crate::application::{ActivePane, AppState, PanelMode, PanelState, QuickViewMode};
use crate::infrastructure::{
    FdSearchAdapter, RipGrepAdapter, StdFileSystem, SystemClipboard, SystemOpenAdapter,
};
use crate::presentation::{FOOTER_ROWS, HEADER_ROWS, TerminalRenderer};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

pub struct ContextMenuState {
    pub actions: Vec<(&'static str, ContextMenuAction)>,
    pub selected: usize,
    pub row: u16,
    pub col: u16,
    pub target: PathBuf,
    pub target_name: String,
}

#[derive(Clone)]
pub enum ContextMenuAction {
    Open,
    QuickView,
    Rename,
    Delete,
    CopyPath,
    OpenVsCode,
}

pub enum ContextMenuResponse {
    Handled,
    Close,
    Execute {
        action: ContextMenuAction,
        target: PathBuf,
        name: String,
    },
}

pub enum MouseOutcome {
    Nothing,
    Redraw,
    ExecuteContext {
        action: ContextMenuAction,
        target: PathBuf,
        name: String,
    },
}

pub struct CopyMoveState {
    pub sources: Vec<PathBuf>,
    pub dest: Input,
}

/// A conflict the background transfer is blocked on, waiting for the user's choice.
pub struct PendingConflict {
    pub path: PathBuf,
    pub reply: Sender<ConflictResolution>,
}

/// UI-side view of an in-progress background copy/move.
pub struct TransferUiState {
    pub kind: TransferKind,
    pub done: usize,
    pub total: usize,
    pub current: PathBuf,
    pub cancel: Arc<AtomicBool>,
    pub pending_conflict: Option<PendingConflict>,
    pub cancelling: bool,
}

#[derive(Debug)]
pub enum Message {
    DrawFiles {
        pane: ActivePane,
        base_path: PathBuf,
        files: Vec<String>,
    },
    QuickViewResult {
        pane: ActivePane,
        mode: QuickViewMode,
    },
    Transfer(transfer::TransferEvent),
}

const MAX_QUERY_LEN: usize = 64;
const MAX_REPEAT_RUN: usize = 16;

#[derive(PartialEq)]
struct PanelUiState {
    current_path: PathBuf,
    cursor: usize,
    scroll: usize,
    mode: PanelMode,
    filter_string: String,
    notification: Option<String>,
    multi_selected_count: usize,
}

impl PanelUiState {
    fn capture(panel: &PanelState) -> Self {
        Self {
            current_path: panel.current_path.clone(),
            cursor: panel.cursor,
            scroll: panel.scroll,
            mode: panel.mode.clone(),
            filter_string: panel.filter_string.clone(),
            notification: panel.notification.clone(),
            multi_selected_count: panel.multi_selected_count(),
        }
    }
}

pub fn is_quit(event: KeyEvent) -> bool {
    event.code == KeyCode::F(10)
        || (event.code == KeyCode::Char('c') && event.modifiers.contains(KeyModifiers::CONTROL))
}

pub fn handle_normal_mode(
    event: KeyEvent,
    fs: &StdFileSystem,
    open: &SystemOpenAdapter,
    clipboard: &mut SystemClipboard,
    state: &mut AppState,
    renderer: &TerminalRenderer,
    sender: &Sender<Message>,
) -> bool {
    let visible_rows = renderer.visible_rows();
    let pane = state.active_pane;
    let panel = state.active_panel_mut();
    let before = PanelUiState::capture(panel);
    match event.code {
        KeyCode::Char(c) => {
            panel.mode = PanelMode::Filter;
            panel.filter_string.push(c);
            navigate::refresh_entries(fs, panel);
        }
        KeyCode::Backspace => navigate::go_up_one_level(fs, panel),
        _ => handle_panel_navigation(
            event,
            fs,
            open,
            clipboard,
            panel,
            renderer.columns,
            visible_rows,
            pane,
            sender,
        ),
    }
    PanelUiState::capture(panel) != before
}

pub fn handle_filter_mode(
    event: KeyEvent,
    fs: &StdFileSystem,
    open: &SystemOpenAdapter,
    clipboard: &mut SystemClipboard,
    state: &mut AppState,
    renderer: &TerminalRenderer,
    sender: &Sender<Message>,
) -> bool {
    let visible_rows = renderer.visible_rows();
    let pane = state.active_pane;
    let panel = state.active_panel_mut();
    let before = PanelUiState::capture(panel);
    match event.code {
        KeyCode::Esc => {
            panel.mode = PanelMode::Normal;
            panel.filter_string.clear();
            navigate::refresh_entries(fs, panel);
        }
        KeyCode::Backspace => {
            panel.filter_string.pop();
            navigate::refresh_entries(fs, panel);
        }
        KeyCode::Char(c) => {
            panel.filter_string.push(c);
            navigate::refresh_entries(fs, panel);
        }
        _ => handle_panel_navigation(
            event,
            fs,
            open,
            clipboard,
            panel,
            renderer.columns,
            visible_rows,
            pane,
            sender,
        ),
    }
    PanelUiState::capture(panel) != before
}

fn handle_panel_navigation(
    event: KeyEvent,
    fs: &StdFileSystem,
    open: &SystemOpenAdapter,
    clipboard: &mut SystemClipboard,
    panel: &mut PanelState,
    columns: u16,
    visible_rows: u16,
    pane: ActivePane,
    sender: &Sender<Message>,
) {
    match event.code {
        KeyCode::Up => navigate::move_cursor(panel, -1, visible_rows),
        KeyCode::Down => navigate::move_cursor(panel, 1, visible_rows),
        KeyCode::Home => navigate::navigate_home(panel),
        KeyCode::End => navigate::navigate_end(panel, visible_rows),
        KeyCode::Enter => handle_enter(event, fs, open, clipboard, panel),
        KeyCode::F(3) => schedule_quick_view(panel, pane, columns, sender),
        _ => {}
    }
}

pub fn schedule_quick_view(
    panel: &mut PanelState,
    pane: ActivePane,
    columns: u16,
    sender: &Sender<Message>,
) {
    if let Some(path) = panel.get_selected_path() {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("Loading {}", name))
            .unwrap_or_else(|| format!("Loading {}", path.display()));
        panel.mode = PanelMode::QuickView(QuickViewMode::Loading { message: file_name });
        let sender = sender.clone();
        thread::spawn(move || {
            let mode = quick_view::preview(path, columns);
            let _ = sender.send(Message::QuickViewResult { pane, mode });
        });
    }
}

fn handle_enter(
    event: KeyEvent,
    fs: &StdFileSystem,
    open: &SystemOpenAdapter,
    clipboard: &mut SystemClipboard,
    panel: &mut PanelState,
) {
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        let absolute = event.modifiers.contains(KeyModifiers::SHIFT);
        let _ = file_ops::copy_to_clipboard(clipboard, panel, absolute);
    } else if !navigate::enter_selected(fs, panel)
        && let Some(entry) = panel.selected_entry()
        && entry.is_file()
    {
        open.open(&entry.path);
    }
}

pub fn handle_quick_view_mode(
    event: KeyEvent,
    state: &mut AppState,
    renderer: &TerminalRenderer,
) -> bool {
    let panel = state.active_panel_mut();
    let visible_rows = renderer.visible_rows();
    let before = PanelUiState::capture(panel);
    match event.code {
        KeyCode::Up => quick_view::scroll(panel, -1, renderer.rows, HEADER_ROWS, FOOTER_ROWS),
        KeyCode::Down => quick_view::scroll(panel, 1, renderer.rows, HEADER_ROWS, FOOTER_ROWS),
        KeyCode::PageUp => quick_view::scroll(
            panel,
            -(renderer.rows as isize).max(1),
            renderer.rows,
            HEADER_ROWS,
            FOOTER_ROWS,
        ),
        KeyCode::PageDown => quick_view::scroll(
            panel,
            renderer.rows as isize,
            renderer.rows,
            HEADER_ROWS,
            FOOTER_ROWS,
        ),
        KeyCode::Left => {
            navigate::move_cursor(panel, -1, visible_rows);
            quick_view::open(panel, renderer.columns);
        }
        KeyCode::Right => {
            navigate::move_cursor(panel, 1, visible_rows);
            quick_view::open(panel, renderer.columns);
        }
        KeyCode::Esc | KeyCode::F(3) => panel.mode = PanelMode::Normal,
        _ => {}
    }
    PanelUiState::capture(panel) != before
}

pub fn handle_delete_confirmation(
    event: KeyEvent,
    delete_paths: &mut Option<Vec<PathBuf>>,
    fs: &StdFileSystem,
    state: &mut AppState,
) -> bool {
    match event.code {
        KeyCode::Char('y') => {
            if let Some(paths) = delete_paths.take() {
                let refs: Vec<&std::path::Path> = paths.iter().map(PathBuf::as_path).collect();
                let panel = state.active_panel_mut();
                let _ = file_ops::delete_selected(fs, panel, &refs);
                true
            } else {
                false
            }
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            if delete_paths.is_some() {
                *delete_paths = None;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

pub fn handle_find_input(
    event: KeyEvent,
    find_filter: &mut Option<Input>,
    sender: &Sender<Message>,
    fs: &StdFileSystem,
    state: &mut AppState,
) -> bool {
    handle_query_submit(event, find_filter, fs, state, |query, state| {
        let sender = sender.clone();
        let pane = state.active_pane;
        let pwd = state.active_panel().current_path.clone();
        state
            .active_panel_mut()
            .set_notification("Searching...".to_string());
        thread::spawn(move || {
            let results = FdSearchAdapter.find(&query, &pwd);
            let _ = sender.send(Message::DrawFiles {
                pane,
                base_path: pwd,
                files: results,
            });
        });
    })
}

pub fn handle_ripgrep_input(
    event: KeyEvent,
    filter: &mut Option<Input>,
    fs: &StdFileSystem,
    state: &mut AppState,
) -> bool {
    handle_query_submit(event, filter, fs, state, |query, state| {
        let results = RipGrepAdapter.find(&query, &state.active_panel().current_path);
        let panel = state.active_panel_mut();
        let base = panel.current_path.clone();
        navigate::replace_entries_from_search(panel, results, &base);
    })
}

pub fn handle_shell_input(
    event: KeyEvent,
    command: &mut Option<Input>,
    history: &mut Vec<String>,
    history_idx: &mut Option<usize>,
    sender: &Sender<Message>,
    state: &mut AppState,
) -> bool {
    let Some(inp) = command.as_mut() else {
        return false;
    };
    match event.code {
        KeyCode::Enter => {
            let raw = inp.value().trim().to_string();
            *command = None;
            *history_idx = None;
            if raw.is_empty() {
                return true;
            }
            history.push(raw.clone());
            let expanded = expand_shell_variables(&raw, state.active_panel());
            let cwd = state.active_panel().current_path.clone();
            let pane = state.active_pane;
            let panel = state.active_panel_mut();
            panel.mode = PanelMode::QuickView(QuickViewMode::Loading {
                message: raw.clone(),
            });
            let tx = sender.clone();
            thread::spawn(move || {
                let lines = run_shell_command(&expanded, &cwd);
                let _ = tx.send(Message::QuickViewResult {
                    pane,
                    mode: QuickViewMode::Text { lines, start: 0 },
                });
            });
            true
        }
        KeyCode::Esc => {
            *command = None;
            *history_idx = None;
            true
        }
        KeyCode::Up => {
            if !history.is_empty() {
                let new_idx = match *history_idx {
                    None => history.len() - 1,
                    Some(i) => i.saturating_sub(1),
                };
                *history_idx = Some(new_idx);
                *inp = Input::new(history[new_idx].clone());
            }
            true
        }
        KeyCode::Down => {
            match *history_idx {
                None => {}
                Some(i) if i + 1 >= history.len() => {
                    *history_idx = None;
                    *inp = Input::default();
                }
                Some(i) => {
                    let new_idx = i + 1;
                    *history_idx = Some(new_idx);
                    *inp = Input::new(history[new_idx].clone());
                }
            }
            true
        }
        _ => {
            if event.code == KeyCode::Char('c') && event.modifiers.contains(KeyModifiers::CONTROL) {
                *history_idx = None;
            }
            inp.handle_event(&Event::Key(event));
            true
        }
    }
}

fn shell_quote(s: &str) -> String {
    // Single-quote escaping: ' → '\'' prevents all shell expansion inside paths.
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn expand_shell_variables(cmd: &str, panel: &PanelState) -> String {
    let paths: Vec<String> = if panel.multi_selected_count() > 0 {
        panel
            .multi_selected_paths()
            .iter()
            .map(|p| shell_quote(&p.display().to_string()))
            .collect()
    } else if let Some(path) = panel.get_selected_path() {
        vec![shell_quote(&path.display().to_string())]
    } else {
        vec![String::new()]
    };
    let first = paths.first().map(|s| s.as_str()).unwrap_or("''");
    let all = paths.join(" ");
    // ponytail: {1}/{@} tokens avoid colliding with $1/$@ in awk/sed/perl programs.
    // Single-pass scan applies both substitutions to the original cmd with no chaining.
    let mut result = String::with_capacity(cmd.len() + all.len());
    let mut remaining = cmd;
    while !remaining.is_empty() {
        if remaining.starts_with("{1}") {
            result.push_str(first);
            remaining = &remaining[3..];
        } else if remaining.starts_with("{@}") {
            result.push_str(&all);
            remaining = &remaining[3..];
        } else {
            let c = remaining.chars().next().unwrap();
            result.push(c);
            remaining = &remaining[c.len_utf8()..];
        }
    }
    result
}

fn run_shell_command(cmd: &str, cwd: &Path) -> Vec<String> {
    let output = std::process::Command::new("sh")
        .args(["-c", cmd])
        .current_dir(cwd)
        .output();
    match output {
        Ok(out) => {
            let mut lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(String::from)
                .collect();
            if !out.stderr.is_empty() {
                if !lines.is_empty() {
                    lines.push(String::new());
                }
                lines.push("── stderr ──".into());
                lines.extend(
                    String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .map(String::from),
                );
            }
            if !out.status.success() {
                let code = out.status.code().map_or("?".to_string(), |c| c.to_string());
                if !lines.is_empty() {
                    lines.push(String::new());
                }
                lines.push(format!("── exit {} ──", code));
            }
            if lines.is_empty() {
                vec!["(no output)".into()]
            } else {
                lines
            }
        }
        Err(e) => vec![format!("Error: {}", e)],
    }
}

fn handle_query_submit(
    event: KeyEvent,
    input: &mut Option<Input>,
    fs: &StdFileSystem,
    state: &mut AppState,
    mut on_submit: impl FnMut(String, &mut AppState),
) -> bool {
    let Some(inp) = input.as_mut() else {
        return false;
    };
    match event.code {
        KeyCode::Enter => {
            match validate_query(inp.value()) {
                Ok(query) => {
                    on_submit(query, state);
                    *input = None;
                }
                Err(message) => state.active_panel_mut().set_notification(message),
            }
            true
        }
        KeyCode::Esc => {
            *input = None;
            navigate::refresh_entries(fs, state.active_panel_mut());
            true
        }
        _ => {
            inp.handle_event(&Event::Key(event));
            true
        }
    }
}

fn validate_query(raw: &str) -> Result<String, String> {
    let query = raw.trim();
    if query.is_empty() {
        return Err("Query is empty".to_string());
    }

    let length = query.chars().count();
    if length > MAX_QUERY_LEN {
        return Err(format!("Query too long (max {})", MAX_QUERY_LEN));
    }

    if query.chars().any(|c| {
        matches!(
            c,
            '*' | '+' | '?' | '|' | '{' | '}' | '(' | ')' | '[' | ']' | '\\' | '^' | '$'
        )
    }) {
        return Err("Query contains unsupported regex tokens".to_string());
    }

    if has_absurd_repeat_run(query) {
        return Err("Query looks too repetitive".to_string());
    }

    Ok(query.to_string())
}

fn has_absurd_repeat_run(query: &str) -> bool {
    let (mut last, mut run) = ('\0', 0usize);
    query.chars().any(|c| {
        run = if c == last { run + 1 } else { 1 };
        last = c;
        run >= MAX_REPEAT_RUN
    })
}

pub fn handle_favorites_input(
    event: KeyEvent,
    active: &mut bool,
    items: &[String],
    selected: &mut usize,
    fs: &StdFileSystem,
    state: &mut AppState,
) -> bool {
    match event.code {
        KeyCode::Up => {
            let next = selected.saturating_sub(1);
            let changed = next != *selected;
            *selected = next;
            changed
        }
        KeyCode::Down => {
            let next = (*selected + 1).min(items.len().saturating_sub(1));
            let changed = next != *selected;
            *selected = next;
            changed
        }
        KeyCode::Enter => {
            if let Some(item) = items.get(*selected) {
                navigate::change_directory(fs, state.active_panel_mut(), PathBuf::from(item));
                *active = false;
                true
            } else {
                false
            }
        }
        KeyCode::Esc => {
            if *active {
                *active = false;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

pub fn handle_mouse_event(
    event: MouseEvent,
    app_state: &mut AppState,
    renderer_rows: u16,
    renderer_cols: u16,
    context_menu: &mut Option<ContextMenuState>,
    sender: &Sender<Message>,
) -> MouseOutcome {
    if let Some(ctx) = context_menu.as_ref() {
        if matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            // Check if click lands on a menu item.
            if let Some(outcome) =
                menu_click_hit(event.row, event.column, ctx, renderer_rows, renderer_cols)
            {
                *context_menu = None;
                return outcome;
            }
            // Click outside menu: close it and handle as normal file-list click.
            *context_menu = None;
            return if mouse_click(
                event.row,
                event.column,
                app_state,
                renderer_rows,
                renderer_cols,
            ) {
                MouseOutcome::Redraw
            } else {
                MouseOutcome::Redraw // redraw to remove menu even if cursor didn't move
            };
        } else if matches!(
            event.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            // Scroll while menu open: close menu and scroll.
            *context_menu = None;
        } else {
            return MouseOutcome::Nothing;
        }
    }

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse_click(
                event.row,
                event.column,
                app_state,
                renderer_rows,
                renderer_cols,
            ) {
                MouseOutcome::Redraw
            } else {
                MouseOutcome::Nothing
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            if mouse_right_click(
                event.row,
                event.column,
                app_state,
                renderer_rows,
                renderer_cols,
                context_menu,
                sender,
            ) {
                MouseOutcome::Redraw
            } else {
                MouseOutcome::Nothing
            }
        }
        MouseEventKind::ScrollUp => {
            if mouse_scroll(app_state, -3, renderer_rows, sender) {
                MouseOutcome::Redraw
            } else {
                MouseOutcome::Nothing
            }
        }
        MouseEventKind::ScrollDown => {
            if mouse_scroll(app_state, 3, renderer_rows, sender) {
                MouseOutcome::Redraw
            } else {
                MouseOutcome::Nothing
            }
        }
        _ => MouseOutcome::Nothing,
    }
}

fn menu_click_hit(
    click_row: u16,
    click_col: u16,
    ctx: &ContextMenuState,
    renderer_rows: u16,
    renderer_cols: u16,
) -> Option<MouseOutcome> {
    let width = ctx.actions.iter().map(|(l, _)| l.len()).max().unwrap_or(8) as u16 + 4;
    let height = ctx.actions.len() as u16 + 2;
    let menu_col = ctx.col.min(renderer_cols.saturating_sub(width));
    let menu_row = ctx.row.min(renderer_rows.saturating_sub(height));

    if click_col < menu_col || click_col >= menu_col + width {
        return None;
    }
    if click_row <= menu_row || click_row >= menu_row + height - 1 {
        return None; // border rows
    }
    let item_idx = (click_row - menu_row - 1) as usize;
    let (_, action) = ctx.actions.get(item_idx)?.clone();
    Some(MouseOutcome::ExecuteContext {
        action,
        target: ctx.target.clone(),
        name: ctx.target_name.clone(),
    })
}

fn mouse_click(
    row: u16,
    col: u16,
    app_state: &mut AppState,
    renderer_rows: u16,
    renderer_cols: u16,
) -> bool {
    use crate::application::ActivePane;
    use crate::presentation::{FOOTER_ROWS, HEADER_ROWS};

    if row < HEADER_ROWS || row >= renderer_rows.saturating_sub(FOOTER_ROWS) {
        return false;
    }
    let list_row = (row - HEADER_ROWS) as usize;

    if app_state.two_pane_mode {
        let pw = renderer_cols.saturating_sub(1) / 2;
        if col == pw {
            return false; // separator
        }
        let pane = if col < pw {
            ActivePane::Left
        } else {
            ActivePane::Right
        };
        let (scroll, len) = match pane {
            ActivePane::Left => (
                app_state.left_panel.scroll,
                app_state.left_panel.entries.len(),
            ),
            ActivePane::Right => (
                app_state.right_panel.scroll,
                app_state.right_panel.entries.len(),
            ),
        };
        app_state.active_pane = pane;
        let idx = scroll + list_row;
        if idx < len {
            match pane {
                ActivePane::Left => app_state.left_panel.cursor = idx,
                ActivePane::Right => app_state.right_panel.cursor = idx,
            }
        }
    } else {
        let scroll = app_state.active_panel().scroll;
        let len = app_state.active_panel().entries.len();
        let idx = scroll + list_row;
        if idx >= len {
            return false;
        }
        app_state.active_panel_mut().cursor = idx;
    }
    true
}

fn mouse_right_click(
    row: u16,
    col: u16,
    app_state: &mut AppState,
    renderer_rows: u16,
    renderer_cols: u16,
    context_menu: &mut Option<ContextMenuState>,
    _sender: &Sender<Message>,
) -> bool {
    use crate::application::ActivePane;
    use crate::presentation::{FOOTER_ROWS, HEADER_ROWS};

    if row < HEADER_ROWS || row >= renderer_rows.saturating_sub(FOOTER_ROWS) {
        return false;
    }
    let list_row = (row - HEADER_ROWS) as usize;

    // Determine pane + index, avoiding long-lived borrows across the pane switch.
    let (new_pane, scroll, len) = if app_state.two_pane_mode {
        let pw = renderer_cols.saturating_sub(1) / 2;
        if col == pw {
            return false;
        }
        let pane = if col < pw {
            ActivePane::Left
        } else {
            ActivePane::Right
        };
        let (s, l) = match pane {
            ActivePane::Left => (
                app_state.left_panel.scroll,
                app_state.left_panel.entries.len(),
            ),
            ActivePane::Right => (
                app_state.right_panel.scroll,
                app_state.right_panel.entries.len(),
            ),
        };
        (Some(pane), s, l)
    } else {
        (
            None,
            app_state.active_panel().scroll,
            app_state.active_panel().entries.len(),
        )
    };

    let idx = scroll + list_row;
    if idx >= len {
        return false;
    }

    if let Some(pane) = new_pane {
        app_state.active_pane = pane;
    }

    // Don't show context menu in quick view mode.
    if matches!(app_state.active_panel().mode, PanelMode::QuickView(_)) {
        return false;
    }

    app_state.active_panel_mut().cursor = idx;

    // Collect entry info before building the menu (releases panel borrow).
    let (target, target_name, is_file) = {
        let entry = &app_state.active_panel().entries[idx];
        (entry.path.clone(), entry.name.clone(), entry.is_file())
    };

    let mut actions: Vec<(&'static str, ContextMenuAction)> = vec![
        ("Open", ContextMenuAction::Open),
        ("Quick View", ContextMenuAction::QuickView),
        ("Rename", ContextMenuAction::Rename),
        ("Delete", ContextMenuAction::Delete),
        ("Copy Path", ContextMenuAction::CopyPath),
    ];
    if is_file {
        actions.push(("Open in VS Code", ContextMenuAction::OpenVsCode));
    }

    *context_menu = Some(ContextMenuState {
        actions,
        selected: 0,
        row,
        col,
        target,
        target_name,
    });
    true
}

fn mouse_scroll(
    app_state: &mut AppState,
    delta: isize,
    renderer_rows: u16,
    sender: &Sender<Message>,
) -> bool {
    use crate::presentation::{FOOTER_ROWS as FR, HEADER_ROWS as HR};
    let visible_rows = renderer_rows.saturating_sub(HR + FR);
    let is_quick_view = matches!(app_state.active_panel().mode, PanelMode::QuickView(_));
    let pane = app_state.active_pane;
    let panel = app_state.active_panel_mut();
    if is_quick_view {
        quick_view::scroll(panel, delta, renderer_rows, HR, FR);
        // In quick view, scroll also schedules the next item preview if navigating
    } else {
        navigate::move_cursor(panel, delta, visible_rows);
    }
    // Reschedule quick view if it was already open (e.g. scroll through files)
    if !is_quick_view {
        if let PanelMode::QuickView(_) = panel.mode.clone() {
            // panel just entered quick view from scroll — shouldn't happen via move_cursor, skip
        }
    }
    let _ = (sender, pane); // suppress unused warnings
    true
}

pub fn handle_context_menu_input(
    event: KeyEvent,
    ctx: &mut Option<ContextMenuState>,
) -> ContextMenuResponse {
    let menu = ctx.as_mut().expect("context menu must be Some");
    match event.code {
        KeyCode::Up => {
            menu.selected = menu.selected.saturating_sub(1);
            ContextMenuResponse::Handled
        }
        KeyCode::Down => {
            menu.selected = (menu.selected + 1).min(menu.actions.len().saturating_sub(1));
            ContextMenuResponse::Handled
        }
        KeyCode::Enter => {
            let (_, action) = menu.actions[menu.selected].clone();
            let target = menu.target.clone();
            let name = menu.target_name.clone();
            *ctx = None;
            ContextMenuResponse::Execute {
                action,
                target,
                name,
            }
        }
        KeyCode::Esc | KeyCode::F(10) => {
            *ctx = None;
            ContextMenuResponse::Close
        }
        _ => ContextMenuResponse::Handled,
    }
}

pub fn handle_rename_input(
    event: KeyEvent,
    rename_input: &mut Option<Input>,
    fs: &StdFileSystem,
    state: &mut AppState,
) -> bool {
    let Some(inp) = rename_input.as_mut() else {
        return false;
    };
    match event.code {
        KeyCode::Enter => {
            let new_name = inp.value().trim().to_string();
            *rename_input = None;
            if new_name.is_empty() {
                return true;
            }
            let panel = state.active_panel_mut();
            if let Some(from) = panel.get_selected_path() {
                let to = from
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("/"))
                    .join(&new_name);
                match fs.rename(&from, &to) {
                    Ok(()) => {
                        navigate::refresh_entries(fs, panel);
                        if let Some(pos) = panel.entries.iter().position(|e| e.name == new_name) {
                            panel.cursor = pos;
                        }
                    }
                    Err(e) => panel.set_notification(format!("Rename failed: {}", e)),
                }
            }
            true
        }
        KeyCode::Esc => {
            *rename_input = None;
            true
        }
        _ => {
            inp.handle_event(&Event::Key(event));
            true
        }
    }
}

pub fn handle_new_folder_input(
    event: KeyEvent,
    new_folder_input: &mut Option<Input>,
    fs: &StdFileSystem,
    state: &mut AppState,
) -> bool {
    let Some(inp) = new_folder_input.as_mut() else {
        return false;
    };
    match event.code {
        KeyCode::Enter => {
            let name = inp.value().trim().to_string();
            *new_folder_input = None;
            if name.is_empty() {
                return true;
            }
            let panel = state.active_panel_mut();
            let parent = panel.current_path.clone();
            match new_folder::create_folder(&name, &parent, fs) {
                Ok(()) => {
                    navigate::refresh_entries(fs, panel);
                    if let Some(pos) = panel.entries.iter().position(|e| e.name == name) {
                        panel.cursor = pos;
                    }
                }
                Err(e) => panel.set_notification(format!("New folder failed: {}", e)),
            }
            true
        }
        KeyCode::Esc => {
            *new_folder_input = None;
            true
        }
        _ => {
            inp.handle_event(&Event::Key(event));
            true
        }
    }
}

/// Spawns a background transfer job and wires its events back through `sender` as
/// `Message::Transfer`. `StdFileSystem` is a zero-sized adapter, cheap to hand to the thread.
fn start_transfer(
    kind: TransferKind,
    sources: Vec<PathBuf>,
    dest: PathBuf,
    sender: &Sender<Message>,
) -> TransferUiState {
    let tx = sender.clone();
    let cancel = transfer::spawn_transfer(StdFileSystem, kind, sources, dest, move |ev| {
        let _ = tx.send(Message::Transfer(ev));
    });
    TransferUiState {
        kind,
        done: 0,
        total: 0,
        current: PathBuf::new(),
        cancel,
        pending_conflict: None,
        cancelling: false,
    }
}

pub fn handle_copy_dest_input(
    event: KeyEvent,
    copy_dest: &mut Option<CopyMoveState>,
    transfer_state: &mut Option<TransferUiState>,
    sender: &Sender<Message>,
) -> bool {
    let Some(cms) = copy_dest.as_mut() else {
        return false;
    };
    match event.code {
        KeyCode::Enter => {
            let sources = cms.sources.clone();
            let dest = PathBuf::from(cms.dest.value().trim());
            *copy_dest = None;
            if dest.as_os_str().is_empty() {
                return true;
            }
            *transfer_state = Some(start_transfer(TransferKind::Copy, sources, dest, sender));
            true
        }
        KeyCode::Esc => {
            *copy_dest = None;
            true
        }
        _ => {
            cms.dest.handle_event(&Event::Key(event));
            true
        }
    }
}

pub fn handle_move_dest_input(
    event: KeyEvent,
    move_dest: &mut Option<CopyMoveState>,
    transfer_state: &mut Option<TransferUiState>,
    sender: &Sender<Message>,
    state: &mut AppState,
) -> bool {
    let Some(cms) = move_dest.as_mut() else {
        return false;
    };
    match event.code {
        KeyCode::Enter => {
            let sources = cms.sources.clone();
            let dest = PathBuf::from(cms.dest.value().trim());
            *move_dest = None;
            if dest.as_os_str().is_empty() {
                return true;
            }
            state.active_panel_mut().clear_multi_selection();
            *transfer_state = Some(start_transfer(TransferKind::Move, sources, dest, sender));
            true
        }
        KeyCode::Esc => {
            *move_dest = None;
            true
        }
        _ => {
            cms.dest.handle_event(&Event::Key(event));
            true
        }
    }
}

/// Handles input while a background transfer is active: conflict-prompt choices,
/// or `Esc` to request cancellation. Returns whether a redraw is needed.
pub fn handle_transfer_input(
    event: KeyEvent,
    transfer_state: &mut Option<TransferUiState>,
) -> bool {
    let Some(t) = transfer_state.as_mut() else {
        return false;
    };
    if let Some(conflict) = t.pending_conflict.take() {
        let resolution = match event.code {
            KeyCode::Char('o') | KeyCode::Char('O') => Some(ConflictResolution::Overwrite),
            KeyCode::Char('s') | KeyCode::Char('S') => Some(ConflictResolution::Skip),
            KeyCode::Char('a') | KeyCode::Char('A') => Some(ConflictResolution::OverwriteAll),
            KeyCode::Char('l') | KeyCode::Char('L') => Some(ConflictResolution::SkipAll),
            KeyCode::Esc => Some(ConflictResolution::Cancel),
            _ => None,
        };
        match resolution {
            Some(r) => {
                let _ = conflict.reply.send(r);
                true
            }
            None => {
                t.pending_conflict = Some(conflict);
                false
            }
        }
    } else if event.code == KeyCode::Esc && !t.cancelling {
        t.cancel.store(true, Ordering::Relaxed);
        t.cancelling = true;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    #[test]
    fn test_validate_query_accepts_trimmed() {
        let result = validate_query("  hello  ");
        assert_eq!(result.unwrap(), "hello".to_string());
    }

    #[test]
    fn test_validate_query_rejects_empty() {
        let result = validate_query("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_query_rejects_too_long() {
        let long = "a".repeat(MAX_QUERY_LEN + 1);
        let result = validate_query(&long);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_query_rejects_regex_tokens() {
        let result = validate_query("a+b");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_query_rejects_absurd_repeat() {
        let long = "a".repeat(MAX_REPEAT_RUN);
        let result = validate_query(&long);
        assert!(result.is_err());
    }

    #[test]
    fn test_has_absurd_repeat_run_false() {
        assert!(!has_absurd_repeat_run("abcabc"));
    }

    #[test]
    fn test_handle_favorites_input_ignored_key_is_not_dirty() {
        let fs = StdFileSystem;
        let mut state = AppState::new(false);
        let mut active = true;
        let items = vec!["/tmp".to_string()];
        let mut selected = 0;

        assert!(!handle_favorites_input(
            KeyEvent::from(KeyCode::Char('x')),
            &mut active,
            &items,
            &mut selected,
            &fs,
            &mut state
        ));
    }

    #[test]
    fn test_handle_delete_confirmation_ignored_key_is_not_dirty() {
        let fs = StdFileSystem;
        let mut state = AppState::new(false);
        let mut delete_paths = Some(vec![PathBuf::from("/tmp/file.txt")]);

        assert!(!handle_delete_confirmation(
            KeyEvent::from(KeyCode::Char('x')),
            &mut delete_paths,
            &fs,
            &mut state
        ));
    }
}
