//! Input handler - keyboard event processing.

use crate::application::ports::FileSystemPort;
use crate::application::use_cases::transfer::{self, ConflictResolution, TransferKind};
use crate::application::use_cases::{dir_size, file_ops, navigate, quick_view};
use crate::application::{ActivePane, AppState, PanelMode, PanelState, QuickViewMode, SizeFigure};
use crate::infrastructure::{
    FdSearchAdapter, RipGrepAdapter, StdFileSystem, SystemClipboard, SystemOpenAdapter,
};
use crate::presentation::{FOOTER_ROWS, HEADER_ROWS, TerminalRenderer};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
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
    DiskUsage {
        pane: ActivePane,
        path: PathBuf,
        usage: Option<(u64, u64)>,
    },
    DeleteDone {
        pane: ActivePane,
        path: PathBuf,
        result: Result<(), String>,
    },
    DrawFiles {
        pane: ActivePane,
        base_path: PathBuf,
        files: Vec<String>,
        generation: u64,
        match_lines: std::collections::HashMap<PathBuf, usize>,
    },
    DirectoryLoaded {
        pane: ActivePane,
        path: PathBuf,
        entries: Vec<crate::domain::FileEntry>,
        generation: u64,
    },
    QuickViewResult {
        pane: ActivePane,
        generation: u64,
        mode: QuickViewMode,
    },
    Transfer(transfer::TransferEvent),
    DirTotalResult {
        pane: ActivePane,
        generation: u64,
        total: u64,
    },
    SelectionTotalResult {
        pane: ActivePane,
        generation: u64,
        total: u64,
    },
}

/// All modal-overlay and feature state threaded through the event loop.
/// Bundled so `run`/`route_input`/`redraw_current_view` take one argument
/// instead of the same fifteen-field tail at every call site.
#[derive(Default)]
pub struct UiState {
    pub delete_paths: Option<Vec<PathBuf>>,
    pub find_filter: Option<Input>,
    pub ripgrep_filter: Option<Input>,
    pub shell_command: Option<Input>,
    pub shell_history: Vec<String>,
    pub shell_history_idx: Option<usize>,
    pub rename_input: Option<Input>,
    pub new_folder_input: Option<Input>,
    pub copy_dest: Option<CopyMoveState>,
    pub transfer_state: Option<TransferUiState>,
    pub move_dest: Option<CopyMoveState>,
    pub context_menu: Option<ContextMenuState>,
    pub favorites_active: bool,
    pub favorites_items: Vec<String>,
    pub favorites_selected: usize,
    pub recent_active: bool,
    pub recent_items: Vec<String>,
    pub recent_selected: usize,
}

/// The overlay currently owning input, in dispatch priority order. `active_overlay()`
/// is the single source of truth for both input routing (`handle_modal_overlays`) and
/// drawing (`redraw_current_view`), so the two can never drift out of sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Transfer,
    Favorites,
    Recent,
    Delete,
    Find,
    Ripgrep,
    Shell,
    Rename,
    NewFolder,
    CopyDest,
    MoveDest,
    ContextMenu,
}

impl UiState {
    /// The overlay currently owning input, if any, in dispatch priority order.
    pub fn active_overlay(&self) -> Option<OverlayKind> {
        if self.transfer_state.is_some() {
            return Some(OverlayKind::Transfer);
        }
        if self.favorites_active {
            return Some(OverlayKind::Favorites);
        }
        if self.recent_active {
            return Some(OverlayKind::Recent);
        }
        if self.delete_paths.is_some() {
            return Some(OverlayKind::Delete);
        }
        if self.find_filter.is_some() {
            return Some(OverlayKind::Find);
        }
        if self.ripgrep_filter.is_some() {
            return Some(OverlayKind::Ripgrep);
        }
        if self.shell_command.is_some() {
            return Some(OverlayKind::Shell);
        }
        if self.rename_input.is_some() {
            return Some(OverlayKind::Rename);
        }
        if self.new_folder_input.is_some() {
            return Some(OverlayKind::NewFolder);
        }
        if self.copy_dest.is_some() {
            return Some(OverlayKind::CopyDest);
        }
        if self.move_dest.is_some() {
            return Some(OverlayKind::MoveDest);
        }
        if self.context_menu.is_some() {
            return Some(OverlayKind::ContextMenu);
        }
        None
    }

    /// True while any overlay owns input; Esc then cancels it instead of quitting.
    pub fn has_active_overlay(&self) -> bool {
        self.active_overlay().is_some()
    }
}

const MAX_QUERY_LEN: usize = 64;
const MAX_REPEAT_RUN: usize = 16;

#[derive(PartialEq)]
struct PanelUiState {
    current_path: PathBuf,
    cursor: usize,
    scroll: usize,
    mode: (u8, usize),
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
            mode: match &panel.mode {
                PanelMode::Normal => (0, 0),
                PanelMode::Filter => (1, 0),
                PanelMode::QuickView(mode) => (
                    2,
                    match mode {
                        QuickViewMode::Text { start, .. } => *start,
                        _ => 0,
                    },
                ),
            },
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
    let before = PanelUiState::capture(state.active_panel());
    match event.code {
        KeyCode::Char(c) if !event.modifiers.contains(KeyModifiers::CONTROL) => {
            let panel = state.active_panel_mut();
            panel.mode = PanelMode::Filter;
            panel.filter_string.push(c);
            navigate::apply_filter(panel);
        }
        KeyCode::Backspace => load_directory_async(
            state,
            sender,
            state
                .active_panel()
                .current_path
                .parent()
                .unwrap_or(Path::new("/"))
                .to_path_buf(),
        ),
        _ => handle_panel_navigation(event, fs, open, clipboard, state, renderer, sender),
    }
    PanelUiState::capture(state.active_panel()) != before
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
    let before = PanelUiState::capture(state.active_panel());
    match event.code {
        KeyCode::Esc => {
            let panel = state.active_panel_mut();
            panel.mode = PanelMode::Normal;
            panel.filter_string.clear();
            navigate::apply_filter(panel);
        }
        KeyCode::Backspace => {
            let panel = state.active_panel_mut();
            panel.filter_string.pop();
            navigate::apply_filter(panel);
        }
        KeyCode::Char(c) if !event.modifiers.contains(KeyModifiers::CONTROL) => {
            let panel = state.active_panel_mut();
            panel.filter_string.push(c);
            navigate::apply_filter(panel);
        }
        _ => handle_panel_navigation(event, fs, open, clipboard, state, renderer, sender),
    }
    PanelUiState::capture(state.active_panel()) != before
}

fn load_directory_async(state: &mut AppState, sender: &Sender<Message>, path: PathBuf) {
    let pane = state.active_pane;
    let panel = state.active_panel_mut();
    if let Some(cancel) = panel.preview_cancel.take() {
        cancel.store(true, Ordering::Relaxed);
    }
    panel.current_path = path.clone();
    panel.mode = PanelMode::Normal;
    panel.filter_string.clear();
    panel.entries.clear();
    panel.search_match_lines.clear();
    if let Some(cancel) = panel.search_cancel.take() {
        cancel.store(true, Ordering::Relaxed);
    }
    if let Some(cancel) = panel.directory_cancel.take() {
        cancel.store(true, Ordering::Relaxed);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    panel.directory_cancel = Some(cancel.clone());
    panel.search_generation += 1;
    let generation = panel.search_generation;
    enqueue_directory(DirectoryJob {
        path,
        pane,
        generation,
        cancel,
        sender: sender.clone(),
    });
}

struct DirectoryJob {
    path: PathBuf,
    pane: ActivePane,
    generation: u64,
    cancel: Arc<AtomicBool>,
    sender: Sender<Message>,
}
struct DirectoryQueue {
    pending: Mutex<Option<DirectoryJob>>,
    wake: Condvar,
}
static DIRECTORY_QUEUE: OnceLock<Arc<DirectoryQueue>> = OnceLock::new();

fn enqueue_directory(job: DirectoryJob) {
    let queue = DIRECTORY_QUEUE.get_or_init(|| {
        let queue = Arc::new(DirectoryQueue {
            pending: Mutex::new(None),
            wake: Condvar::new(),
        });
        let worker = queue.clone();
        thread::spawn(move || {
            loop {
                let mut pending = worker.pending.lock().unwrap();
                while pending.is_none() {
                    pending = worker.wake.wait(pending).unwrap();
                }
                let job = pending.take().unwrap();
                drop(pending);
                if job.cancel.load(Ordering::Relaxed) {
                    continue;
                }
                let entries = StdFileSystem.list_dir(&job.path).unwrap_or_default();
                if !job.cancel.load(Ordering::Relaxed) {
                    let _ = job.sender.send(Message::DirectoryLoaded {
                        pane: job.pane,
                        path: job.path,
                        entries,
                        generation: job.generation,
                    });
                }
            }
        });
        queue
    });
    *queue.pending.lock().unwrap() = Some(job);
    queue.wake.notify_one();
}

fn handle_panel_navigation(
    event: KeyEvent,
    _fs: &StdFileSystem,
    open: &SystemOpenAdapter,
    clipboard: &mut SystemClipboard,
    state: &mut AppState,
    renderer: &TerminalRenderer,
    sender: &Sender<Message>,
) {
    let pane = state.active_pane;
    let panel = state.active_panel_mut();
    let columns = renderer.columns;
    let visible_rows = renderer.visible_rows();
    match event.code {
        KeyCode::Up => navigate::move_cursor(panel, -1, visible_rows),
        KeyCode::Down => navigate::move_cursor(panel, 1, visible_rows),
        KeyCode::Home => navigate::navigate_home(panel),
        KeyCode::End => navigate::navigate_end(panel, visible_rows),
        KeyCode::Enter => handle_enter(event, open, clipboard, panel, pane, sender),
        KeyCode::F(3) => schedule_quick_view(panel, pane, columns, sender),
        // Ctrl+P alias for terminals/keyboards where F3 is awkward (macOS media keys).
        KeyCode::Char('p') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            schedule_quick_view(panel, pane, columns, sender)
        }
        _ => {}
    }
}

pub fn schedule_quick_view(
    panel: &mut PanelState,
    pane: ActivePane,
    columns: u16,
    sender: &Sender<Message>,
) {
    if let Some(cancel) = panel.preview_cancel.take() {
        cancel.store(true, Ordering::Relaxed);
    }
    if let Some(path) = panel.get_selected_path() {
        panel.quick_view_generation += 1;
        let generation = panel.quick_view_generation;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("Loading {}", name))
            .unwrap_or_else(|| format!("Loading {}", path.display()));
        panel.mode = PanelMode::QuickView(QuickViewMode::Loading { message: file_name });
        let cancel = Arc::new(AtomicBool::new(false));
        panel.preview_cancel = Some(cancel.clone());
        enqueue_preview(PreviewJob {
            path,
            columns,
            pane,
            generation,
            cancel,
            sender: sender.clone(),
        });
    }
}

struct PreviewJob {
    path: PathBuf,
    columns: u16,
    pane: ActivePane,
    generation: u64,
    cancel: Arc<AtomicBool>,
    sender: Sender<Message>,
}
struct PreviewQueue {
    pending: Mutex<Option<PreviewJob>>,
    wake: Condvar,
}
static PREVIEW_QUEUE: OnceLock<Arc<PreviewQueue>> = OnceLock::new();

fn enqueue_preview(job: PreviewJob) {
    let queue = PREVIEW_QUEUE.get_or_init(|| {
        let queue = Arc::new(PreviewQueue {
            pending: Mutex::new(None),
            wake: Condvar::new(),
        });
        let worker_queue = queue.clone();
        thread::spawn(move || {
            loop {
                let mut pending = worker_queue.pending.lock().unwrap();
                while pending.is_none() {
                    pending = worker_queue.wake.wait(pending).unwrap();
                }
                let job = pending.take().unwrap();
                drop(pending);
                if job.cancel.load(Ordering::Relaxed) {
                    continue;
                }
                let mode = quick_view::preview(job.path, job.columns);
                if !job.cancel.load(Ordering::Relaxed) {
                    let _ = job.sender.send(Message::QuickViewResult {
                        pane: job.pane,
                        generation: job.generation,
                        mode,
                    });
                }
            }
        });
        queue
    });
    *queue.pending.lock().unwrap() = Some(job);
    queue.wake.notify_one();
}

/// Kicks off (or cancels) the background recursive-size jobs for the active panel's
/// selection, called whenever the multi-selection changes (`Space`).
pub fn schedule_size_jobs(app_state: &mut AppState, sender: &Sender<Message>) {
    let pane = app_state.active_pane;
    let panel = app_state.active_panel_mut();

    if let Some(cancel) = panel.selection_cancel.take() {
        cancel.store(true, Ordering::Relaxed);
    }

    if panel.multi_selected_count() == 0 {
        panel.selection_total = SizeFigure::Idle;
        return;
    }
    spawn_selection_total(panel, pane, sender);
    if matches!(panel.dir_total, SizeFigure::Idle) {
        spawn_dir_total(panel, pane, sender);
    }
}

fn spawn_selection_total(panel: &mut PanelState, pane: ActivePane, sender: &Sender<Message>) {
    panel.selection_total_generation += 1;
    let generation = panel.selection_total_generation;
    panel.selection_total = SizeFigure::Computing(generation);
    let cancel = Arc::new(AtomicBool::new(false));
    panel.selection_cancel = Some(cancel.clone());

    let roots: Vec<_> = panel
        .multi_selected
        .iter()
        .filter_map(|&i| panel.entries.get(i))
        .cloned()
        .collect();
    let tx = sender.clone();
    thread::spawn(move || {
        let total = dir_size::total_size(&StdFileSystem, &roots, &cancel);
        let _ = tx.send(Message::SelectionTotalResult {
            pane,
            generation,
            total,
        });
    });
}

fn spawn_dir_total(panel: &mut PanelState, pane: ActivePane, sender: &Sender<Message>) {
    panel.dir_total_generation += 1;
    let generation = panel.dir_total_generation;
    panel.dir_total = SizeFigure::Computing(generation);
    let roots: Vec<_> = panel
        .entries
        .iter()
        .filter(|e| e.name != "..")
        .cloned()
        .collect();
    let tx = sender.clone();
    thread::spawn(move || {
        let total = dir_size::total_size(&StdFileSystem, &roots, &AtomicBool::new(false));
        let _ = tx.send(Message::DirTotalResult {
            pane,
            generation,
            total,
        });
    });
}

fn handle_enter(
    event: KeyEvent,
    open: &SystemOpenAdapter,
    clipboard: &mut SystemClipboard,
    panel: &mut PanelState,
    pane: ActivePane,
    sender: &Sender<Message>,
) {
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        let absolute = event.modifiers.contains(KeyModifiers::SHIFT);
        let _ = file_ops::copy_to_clipboard(clipboard, panel, absolute);
    } else if let Some(entry) = panel.selected_entry().cloned() {
        if entry.name == ".." || entry.is_dir() {
            let path = if entry.name == ".." {
                panel
                    .current_path
                    .parent()
                    .unwrap_or(Path::new("/"))
                    .to_path_buf()
            } else {
                entry.path
            };
            panel.current_path = path.clone();
            panel.mode = PanelMode::Normal;
            panel.filter_string.clear();
            panel.entries.clear();
            panel.search_match_lines.clear();
            if let Some(cancel) = panel.search_cancel.take() {
                cancel.store(true, Ordering::Relaxed);
            }
            if let Some(cancel) = panel.directory_cancel.take() {
                cancel.store(true, Ordering::Relaxed);
            }
            let cancel = Arc::new(AtomicBool::new(false));
            panel.directory_cancel = Some(cancel.clone());
            panel.search_generation += 1;
            let generation = panel.search_generation;
            enqueue_directory(DirectoryJob {
                path,
                pane,
                generation,
                cancel,
                sender: sender.clone(),
            });
        } else if entry.is_file() {
            open.open(&entry.path);
        }
    }
}

pub fn handle_quick_view_mode(
    event: KeyEvent,
    state: &mut AppState,
    renderer: &TerminalRenderer,
    sender: &Sender<Message>,
) -> bool {
    let pane = state.active_pane;
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
            schedule_quick_view(panel, pane, renderer.columns, sender);
        }
        KeyCode::Right => {
            navigate::move_cursor(panel, 1, visible_rows);
            schedule_quick_view(panel, pane, renderer.columns, sender);
        }
        KeyCode::Esc | KeyCode::F(3) => {
            panel.quick_view_generation += 1;
            panel.mode = PanelMode::Normal;
        }
        KeyCode::Char('p') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            panel.quick_view_generation += 1;
            panel.mode = PanelMode::Normal
        }
        _ => {}
    }
    PanelUiState::capture(panel) != before
}

pub fn handle_delete_confirmation(
    event: KeyEvent,
    delete_paths: &mut Option<Vec<PathBuf>>,
    sender: &Sender<Message>,
    state: &mut AppState,
) -> bool {
    match event.code {
        KeyCode::Char('y') => {
            let Some(paths) = delete_paths.take() else {
                return false;
            };
            let pane = state.active_pane;
            let path = state.active_panel().current_path.clone();
            let panel = state.active_panel_mut();
            panel.set_notification("Deleting...".into());
            panel.clear_multi_selection();
            let sender = sender.clone();
            thread::spawn(move || {
                let refs: Vec<_> = paths.iter().map(PathBuf::as_path).collect();
                let result = file_ops::delete_selected(&StdFileSystem, &refs);
                let _ = sender.send(Message::DeleteDone { pane, path, result });
            });
            true
        }
        KeyCode::Char('n') | KeyCode::Esc => delete_paths.take().is_some(),
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
        state.active_panel_mut().search_generation += 1;
        let generation = state.active_panel().search_generation;
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
                generation,
                match_lines: std::collections::HashMap::new(),
            });
        });
    })
}

pub fn handle_ripgrep_input(
    event: KeyEvent,
    filter: &mut Option<Input>,
    fs: &StdFileSystem,
    sender: &Sender<Message>,
    state: &mut AppState,
) -> bool {
    handle_query_submit(event, filter, fs, state, |query, state| {
        let pane = state.active_pane;
        if let Some(cancel) = state.active_panel_mut().search_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        state.active_panel_mut().search_generation += 1;
        let generation = state.active_panel().search_generation;
        let base_path = state.active_panel().current_path.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        state.active_panel_mut().search_cancel = Some(cancel.clone());
        state
            .active_panel_mut()
            .set_notification("Searching...".into());
        let sender = sender.clone();
        thread::spawn(move || {
            let (files, relative_lines) =
                RipGrepAdapter.find_with_cancel_and_lines(&query, &base_path, &cancel);
            let match_lines = relative_lines
                .into_iter()
                .map(|(file, line)| (base_path.join(file), line))
                .collect();
            let _ = sender.send(Message::DrawFiles {
                pane,
                base_path,
                files,
                generation,
                match_lines,
            });
        });
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
            submit_shell_command(&raw, state, sender);
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

/// Runs `raw` via `sh -c` in a background thread, showing its output in quick view.
fn submit_shell_command(raw: &str, state: &mut AppState, sender: &Sender<Message>) {
    let expanded = expand_shell_variables(raw, state.active_panel());
    let cwd = state.active_panel().current_path.clone();
    let pane = state.active_pane;
    let panel = state.active_panel_mut();
    panel.quick_view_generation += 1;
    let generation = panel.quick_view_generation;
    panel.mode = PanelMode::QuickView(QuickViewMode::Loading {
        message: raw.to_string(),
    });
    let tx = sender.clone();
    thread::spawn(move || {
        let lines = run_shell_command(&expanded, &cwd);
        let _ = tx.send(Message::QuickViewResult {
            pane,
            generation,
            mode: QuickViewMode::Text { lines, start: 0 },
        });
    });
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
        KeyCode::Esc if *active => {
            *active = false;
            true
        }
        _ => false,
    }
}

pub fn handle_recent_input(
    event: KeyEvent,
    active: &mut bool,
    items: &[String],
    selected: &mut usize,
    fs: &StdFileSystem,
    state: &mut AppState,
) -> bool {
    match event.code {
        KeyCode::Up => {
            let n = selected.saturating_sub(1);
            let changed = n != *selected;
            *selected = n;
            changed
        }
        KeyCode::Down => {
            let n = (*selected + 1).min(items.len().saturating_sub(1));
            let changed = n != *selected;
            *selected = n;
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
            *active = false;
            true
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
            // Click outside menu: close it, apply the click, and redraw regardless.
            *context_menu = None;
            let _ = mouse_click(
                event.row,
                event.column,
                app_state,
                renderer_rows,
                renderer_cols,
            );
            return MouseOutcome::Redraw;
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
            ) {
                MouseOutcome::Redraw
            } else {
                MouseOutcome::Nothing
            }
        }
        MouseEventKind::ScrollUp => {
            if mouse_scroll(app_state, -3, renderer_rows) {
                MouseOutcome::Redraw
            } else {
                MouseOutcome::Nothing
            }
        }
        MouseEventKind::ScrollDown => {
            if mouse_scroll(app_state, 3, renderer_rows) {
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

/// Row/column hit-test against the file-list area: returns the pane to activate
/// (if two-pane mode), the list index under the click, and that pane's entry count.
/// Callers decide how to treat out-of-range indices (left-click activates a pane
/// even on empty space; right-click does not).
fn hit_test(
    app_state: &AppState,
    row: u16,
    col: u16,
    renderer_rows: u16,
    renderer_cols: u16,
) -> Option<(Option<ActivePane>, usize, usize)> {
    if row < HEADER_ROWS || row >= renderer_rows.saturating_sub(FOOTER_ROWS) {
        return None;
    }
    let list_row = (row - HEADER_ROWS) as usize;

    if app_state.two_pane_mode {
        let pw = renderer_cols.saturating_sub(1) / 2;
        if col == pw {
            return None; // separator
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
        Some((Some(pane), scroll + list_row, len))
    } else {
        let panel = &app_state.active_panel();
        Some((None, panel.scroll + list_row, panel.entries.len()))
    }
}

fn mouse_click(
    row: u16,
    col: u16,
    app_state: &mut AppState,
    renderer_rows: u16,
    renderer_cols: u16,
) -> bool {
    let Some((pane, idx, len)) = hit_test(app_state, row, col, renderer_rows, renderer_cols) else {
        return false;
    };
    if let Some(pane) = pane {
        app_state.active_pane = pane;
    }
    if idx < len {
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
) -> bool {
    let Some((new_pane, idx, len)) = hit_test(app_state, row, col, renderer_rows, renderer_cols)
    else {
        return false;
    };
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
        actions.push(("Open in editor", ContextMenuAction::OpenVsCode));
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

fn mouse_scroll(app_state: &mut AppState, delta: isize, renderer_rows: u16) -> bool {
    use crate::presentation::{FOOTER_ROWS as FR, HEADER_ROWS as HR};
    let visible_rows = renderer_rows.saturating_sub(HR + FR);
    let panel = app_state.active_panel_mut();
    if matches!(panel.mode, PanelMode::QuickView(_)) {
        quick_view::scroll(panel, delta, renderer_rows, HR, FR);
    } else {
        navigate::move_cursor(panel, delta, visible_rows);
    }
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
        KeyCode::Esc => {
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
            match fs
                .create_dir(&parent.join(&name))
                .map_err(|e| e.to_string())
            {
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

/// Handles input for the copy or move destination prompt. Move clears the
/// multi-selection once the job starts; copy leaves it.
pub fn handle_copy_move_dest_input(
    event: KeyEvent,
    dest: &mut Option<CopyMoveState>,
    transfer_state: &mut Option<TransferUiState>,
    sender: &Sender<Message>,
    state: &mut AppState,
    kind: TransferKind,
    clear_selection: bool,
) -> bool {
    let Some(cms) = dest.as_mut() else {
        return false;
    };
    match event.code {
        KeyCode::Enter => {
            let sources = cms.sources.clone();
            let dest_path = PathBuf::from(cms.dest.value().trim());
            *dest = None;
            if dest_path.as_os_str().is_empty() {
                return true;
            }
            if clear_selection {
                state.active_panel_mut().clear_multi_selection();
            }
            *transfer_state = Some(start_transfer(kind, sources, dest_path, sender));
            true
        }
        KeyCode::Esc => {
            *dest = None;
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
    use crate::domain::{EntryKind, FileEntry};
    use crate::test_support::create_temp_dir;
    use crossterm::event::KeyEvent;
    use std::sync::mpsc;

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
    fn test_schedule_quick_view_loads_small_text_in_worker() {
        let base = create_temp_dir("schedule_quick_view_small_text");
        let file_path = base.join("note.txt");
        std::fs::write(&file_path, "hello world").unwrap();
        let mut panel = PanelState::new();
        panel.entries = vec![FileEntry::new(
            file_path.clone(),
            "note.txt".to_string(),
            EntryKind::File,
            11,
        )];
        let (tx, rx) = mpsc::channel();

        schedule_quick_view(&mut panel, ActivePane::Left, 80, &tx);

        assert!(matches!(
            panel.mode,
            PanelMode::QuickView(QuickViewMode::Loading { .. })
        ));
        assert!(matches!(
            rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap(),
            Message::QuickViewResult {
                mode: QuickViewMode::Text { .. },
                generation: 1,
                ..
            }
        ));
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
        let (tx, _rx) = mpsc::channel();
        let mut state = AppState::new(false);
        let mut delete_paths = Some(vec![PathBuf::from("/tmp/file.txt")]);

        assert!(!handle_delete_confirmation(
            KeyEvent::from(KeyCode::Char('x')),
            &mut delete_paths,
            &tx,
            &mut state
        ));
    }
}
