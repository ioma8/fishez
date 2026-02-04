//! Input handler - keyboard event processing.

use crate::application::ports::{FileSystemPort, OpenPort, SearchPort};
use crate::application::use_cases::{file_ops, navigate, quick_view};
use crate::application::{ActivePane, AppState, PanelMode, PanelState};
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
}

const MAX_QUERY_LEN: usize = 64;
const MAX_REPEAT_RUN: usize = 16;
const FORBIDDEN_REGEX_CHARS: &[char] = &[
    '*', '+', '?', '|', '{', '}', '(', ')', '[', ']', '\\', '^', '$',
];

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
) {
    let visible_rows = renderer.visible_rows();
    let panel = state.active_panel_mut();
    match event.code {
        KeyCode::Char(c) => {
            panel.mode = PanelMode::Filter;
            panel.filter_string.push(c);
            navigate::refresh_entries(fs, panel);
        }
        KeyCode::Backspace => navigate::go_up_one_level(fs, panel),
        KeyCode::Up => navigate::move_cursor(panel, -1, visible_rows),
        KeyCode::Down => navigate::move_cursor(panel, 1, visible_rows),
        KeyCode::Home => navigate::navigate_home(panel),
        KeyCode::End => navigate::navigate_end(panel, visible_rows),
        KeyCode::Enter => handle_enter(event, fs, open, clipboard, panel),
        KeyCode::F(3) => quick_view::open(panel, renderer.columns),
        _ => {}
    }
}

pub fn handle_filter_mode(
    event: KeyEvent,
    fs: &StdFileSystem,
    open: &SystemOpenAdapter,
    clipboard: &mut SystemClipboard,
    state: &mut AppState,
    renderer: &TerminalRenderer,
) {
    let visible_rows = renderer.visible_rows();
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
        KeyCode::Up => navigate::move_cursor(panel, -1, visible_rows),
        KeyCode::Down => navigate::move_cursor(panel, 1, visible_rows),
        KeyCode::Home => navigate::navigate_home(panel),
        KeyCode::End => navigate::navigate_end(panel, visible_rows),
        KeyCode::Enter => handle_enter(event, fs, open, clipboard, panel),
        KeyCode::F(3) => quick_view::open(panel, renderer.columns),
        _ => {}
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
        && let Some(path) = panel.get_selected_path()
        && fs.is_file(&path)
    {
        open.open(&path);
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
    if let Some(paths) = delete_paths {
        match event.code {
            KeyCode::Char('y') => {
                for path in paths.iter() {
                    let _ = fs.delete(path);
                }
                *delete_paths = None;
                let panel = state.active_panel_mut();
                panel.clear_multi_selection();
                navigate::refresh_entries(fs, panel);
            }
            KeyCode::Char('n') | KeyCode::Esc => *delete_paths = None,
            _ => {}
        }
    }
}

pub fn handle_find_input(
    event: KeyEvent,
    find_filter: &mut Option<String>,
    sender: &Sender<Message>,
    fs: &StdFileSystem,
    state: &mut AppState,
) {
    if let Some(filter) = find_filter {
        match event.code {
            KeyCode::Char(c) => filter.push(c),
            KeyCode::Backspace => {
                filter.pop();
            }
            KeyCode::Enter => {
                let query = match validate_query(filter) {
                    Ok(query) => query,
                    Err(message) => {
                        state.active_panel_mut().set_notification(message);
                        return;
                    }
                };
                let sender = sender.clone();
                let pane = state.active_pane;
                let pwd = state.active_panel().current_path.clone();
                state
                    .active_panel_mut()
                    .set_notification("Searching...".to_string());
                thread::spawn(move || {
                    let results = FdSearchAdapter::new().find(&query, &pwd);
                    let _ = sender.send(Message::DrawFiles {
                        pane,
                        base_path: pwd,
                        files: results,
                    });
                });
                *find_filter = None;
            }
            KeyCode::Esc => {
                *find_filter = None;
                navigate::refresh_entries(fs, state.active_panel_mut());
            }
            _ => {}
        }
    }
}

pub fn handle_ripgrep_input(
    event: KeyEvent,
    filter: &mut Option<String>,
    fs: &StdFileSystem,
    state: &mut AppState,
) {
    if let Some(f) = filter {
        match event.code {
            KeyCode::Char(c) => f.push(c),
            KeyCode::Backspace => {
                f.pop();
            }
            KeyCode::Enter => {
                let query = match validate_query(f) {
                    Ok(query) => query,
                    Err(message) => {
                        state.active_panel_mut().set_notification(message);
                        return;
                    }
                };
                let results =
                    RipGrepAdapter::new().find(&query, &state.active_panel().current_path);
                let panel = state.active_panel_mut();
                let base = panel.current_path.clone();
                navigate::replace_entries_from_search(panel, results, &base);
                *filter = None;
            }
            KeyCode::Esc => {
                *filter = None;
                navigate::refresh_entries(fs, state.active_panel_mut());
            }
            _ => {}
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

    if query.chars().any(|c| FORBIDDEN_REGEX_CHARS.contains(&c)) {
        return Err("Query contains unsupported regex tokens".to_string());
    }

    if has_absurd_repeat_run(query) {
        return Err("Query looks too repetitive".to_string());
    }

    Ok(query.to_string())
}

fn has_absurd_repeat_run(query: &str) -> bool {
    let mut last: Option<char> = None;
    let mut run = 0usize;
    for ch in query.chars() {
        if Some(ch) == last {
            run += 1;
        } else {
            last = Some(ch);
            run = 1;
        }
        if run >= MAX_REPEAT_RUN {
            return true;
        }
    }
    false
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
