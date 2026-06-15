//! Input handler - keyboard event processing.

use crate::application::use_cases::{file_ops, navigate, quick_view};
use crate::application::{ActivePane, AppState, PanelMode, PanelState, QuickViewMode};
use crate::infrastructure::{
    FdSearchAdapter, RipGrepAdapter, StdFileSystem, SystemClipboard, SystemOpenAdapter,
};
use crate::presentation::{FOOTER_ROWS, HEADER_ROWS, TerminalRenderer};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;

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
}

const MAX_QUERY_LEN: usize = 64;
const MAX_REPEAT_RUN: usize = 16;

enum QueryInputAction {
    None,
    Cancel,
    Submit(String),
    Invalid(String),
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
) {
    let visible_rows = renderer.visible_rows();
    let pane = state.active_pane;
    let panel = state.active_panel_mut();
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
}

pub fn handle_filter_mode(
    event: KeyEvent,
    fs: &StdFileSystem,
    open: &SystemOpenAdapter,
    clipboard: &mut SystemClipboard,
    state: &mut AppState,
    renderer: &TerminalRenderer,
    sender: &Sender<Message>,
) {
    let visible_rows = renderer.visible_rows();
    let pane = state.active_pane;
    let panel = state.active_panel_mut();
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

fn schedule_quick_view(
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

pub fn handle_quick_view_mode(event: KeyEvent, state: &mut AppState, renderer: &TerminalRenderer) {
    let panel = state.active_panel_mut();
    let visible_rows = renderer.visible_rows();
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
}

pub fn handle_delete_confirmation(
    event: KeyEvent,
    delete_paths: &mut Option<Vec<PathBuf>>,
    fs: &StdFileSystem,
    state: &mut AppState,
) {
    match event.code {
        KeyCode::Char('y') => {
            if let Some(paths) = delete_paths.take() {
                let refs: Vec<&std::path::Path> = paths.iter().map(PathBuf::as_path).collect();
                let panel = state.active_panel_mut();
                let _ = file_ops::delete_selected(fs, panel, &refs);
            }
        }
        KeyCode::Char('n') | KeyCode::Esc => *delete_paths = None,
        _ => {}
    }
}

pub fn handle_find_input(
    event: KeyEvent,
    find_filter: &mut Option<String>,
    sender: &Sender<Message>,
    fs: &StdFileSystem,
    state: &mut AppState,
) {
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
    });
}

pub fn handle_ripgrep_input(
    event: KeyEvent,
    filter: &mut Option<String>,
    fs: &StdFileSystem,
    state: &mut AppState,
) {
    handle_query_submit(event, filter, fs, state, |query, state| {
        let results = RipGrepAdapter.find(&query, &state.active_panel().current_path);
        let panel = state.active_panel_mut();
        let base = panel.current_path.clone();
        navigate::replace_entries_from_search(panel, results, &base);
    });
}

fn handle_query_input_event(event: KeyEvent, input: &mut String) -> QueryInputAction {
    match event.code {
        KeyCode::Char(c) => {
            input.push(c);
            QueryInputAction::None
        }
        KeyCode::Backspace => {
            input.pop();
            QueryInputAction::None
        }
        KeyCode::Enter => match validate_query(input) {
            Ok(query) => QueryInputAction::Submit(query),
            Err(message) => QueryInputAction::Invalid(message),
        },
        KeyCode::Esc => QueryInputAction::Cancel,
        _ => QueryInputAction::None,
    }
}

fn handle_query_submit(
    event: KeyEvent,
    input: &mut Option<String>,
    fs: &StdFileSystem,
    state: &mut AppState,
    mut on_submit: impl FnMut(String, &mut AppState),
) {
    let Some(value) = input.as_mut() else {
        return;
    };
    match handle_query_input_event(event, value) {
        QueryInputAction::None => {}
        QueryInputAction::Invalid(message) => state.active_panel_mut().set_notification(message),
        QueryInputAction::Submit(query) => {
            on_submit(query, state);
            *input = None;
        }
        QueryInputAction::Cancel => {
            *input = None;
            navigate::refresh_entries(fs, state.active_panel_mut());
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

    if query.chars().any(|c| matches!(c, '*'|'+'|'?'|'|'|'{'|'}'|'('|')'|'['|']'|'\\'|'^'|'$')) {
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
) {
    match event.code {
        KeyCode::Up => *selected = selected.saturating_sub(1),
        KeyCode::Down => *selected = (*selected + 1).min(items.len().saturating_sub(1)),
        KeyCode::Enter => {
            if let Some(item) = items.get(*selected) {
                navigate::change_directory(fs, state.active_panel_mut(), PathBuf::from(item));
                *active = false;
            }
        }
        KeyCode::Esc => *active = false,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
