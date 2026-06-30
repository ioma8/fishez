//! Input handler - keyboard event processing.

use crate::application::use_cases::{file_ops, navigate, quick_view};
use crate::application::{ActivePane, AppState, PanelMode, PanelState, QuickViewMode};
use crate::infrastructure::{
    FdSearchAdapter, RipGrepAdapter, StdFileSystem, SystemClipboard, SystemOpenAdapter,
};
use crate::presentation::{FOOTER_ROWS, HEADER_ROWS, TerminalRenderer};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};
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
    Edit,
    Cancel,
    Submit(String),
    Invalid(String),
}

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
    find_filter: &mut Option<String>,
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
    filter: &mut Option<String>,
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
    command: &mut Option<String>,
    history: &mut Vec<String>,
    history_idx: &mut Option<usize>,
    sender: &Sender<Message>,
    state: &mut AppState,
) -> bool {
    let Some(value) = command.as_mut() else {
        return false;
    };
    match event.code {
        KeyCode::Char(c) => {
            value.push(c);
            *history_idx = None;
            true
        }
        KeyCode::Backspace => {
            value.pop();
            *history_idx = None;
            true
        }
        KeyCode::Enter => {
            let raw = value.trim().to_string();
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
            panel.mode = PanelMode::QuickView(QuickViewMode::Loading { message: raw.clone() });
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
                *value = history[new_idx].clone();
            }
            true
        }
        KeyCode::Down => {
            match *history_idx {
                None => {}
                Some(i) if i + 1 >= history.len() => {
                    *history_idx = None;
                    value.clear();
                }
                Some(i) => {
                    let new_idx = i + 1;
                    *history_idx = Some(new_idx);
                    *value = history[new_idx].clone();
                }
            }
            true
        }
        _ => false,
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

fn handle_query_input_event(event: KeyEvent, input: &mut String) -> QueryInputAction {
    match event.code {
        KeyCode::Char(c) => {
            input.push(c);
            QueryInputAction::Edit
        }
        KeyCode::Backspace => {
            input.pop();
            QueryInputAction::Edit
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
) -> bool {
    let Some(value) = input.as_mut() else {
        return false;
    };
    match handle_query_input_event(event, value) {
        QueryInputAction::None => false,
        QueryInputAction::Edit => true,
        QueryInputAction::Invalid(message) => {
            state.active_panel_mut().set_notification(message);
            true
        }
        QueryInputAction::Submit(query) => {
            on_submit(query, state);
            *input = None;
            true
        }
        QueryInputAction::Cancel => {
            *input = None;
            navigate::refresh_entries(fs, state.active_panel_mut());
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
