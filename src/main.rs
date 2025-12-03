//! Fishez - A terminal file manager using Clean Architecture.
//!
//! Composition Root: This is where all the layers are wired together.

mod application;
mod domain;
mod infrastructure;
mod presentation;

// Legacy modules (temporarily kept for features not yet migrated)
#[allow(dead_code)]
mod features;
#[allow(dead_code)]
mod files_view;
mod logger;
#[allow(dead_code)]
mod terminal_ui;

use application::use_cases::{file_ops, navigate};
use application::{AppState, PanelMode, QuickViewMode};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::Stylize;
use image::DynamicImage;
use image::load_from_memory;
use infrastructure::{
    FdSearchAdapter, RipGrepAdapter, StdFileSystem, SystemClipboard, SystemOpenAdapter,
    VsCodeAdapter,
};
use little_exif::exif_tag::ExifTag;
use little_exif::metadata::Metadata;
use presentation::{FOOTER_ROWS, HEADER_ROWS, TerminalRenderer};
use std::env;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

use crate::application::ports::{FileSystemPort, OpenPort, SearchPort};

/// Messages for async communication
#[derive(Debug)]
pub enum Message {
    DrawFiles(Vec<String>),
}

fn main() {
    // Panic handler
    std::panic::set_hook(Box::new(|panic_info| {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("err.txt")
        {
            let _ = writeln!(file, "panic: {:?}", panic_info);
        }
    }));

    // Parse arguments
    let two_pane = env::args().any(|arg| arg == "--two-pane" || arg == "-2");

    // Initialize infrastructure (adapters)
    let fs_adapter = StdFileSystem::new();
    let mut clipboard_adapter = SystemClipboard::new();
    let open_adapter = SystemOpenAdapter::new();
    let vscode_adapter = VsCodeAdapter::new();
    let fd_search = FdSearchAdapter::new();
    let rg_search = RipGrepAdapter::new();

    // Initialize application state
    let mut app_state = AppState::new(two_pane);

    // Refresh entries for both panels
    navigate::refresh_entries(&fs_adapter, &mut app_state.left_panel);
    navigate::refresh_entries(&fs_adapter, &mut app_state.right_panel);

    // Initialize presentation (renderer)
    let mut renderer = TerminalRenderer::new();

    // Message channel for async operations
    let (sender, receiver): (Sender<Message>, Receiver<Message>) = mpsc::channel();

    // Feature-specific state (for features not yet fully migrated)
    let mut delete_paths: Option<Vec<PathBuf>> = None;
    let mut find_filter: Option<String> = None;
    let mut ripgrep_filter: Option<String> = None;
    let mut favorites_active = false;
    let mut favorites_items: Vec<String> = load_favorites();
    let mut favorites_selected: usize = 0;

    // Initial draw
    draw(&mut renderer, &app_state);

    // Event loop
    loop {
        if event::poll(std::time::Duration::from_millis(100)).unwrap() {
            match event::read().unwrap() {
                Event::Key(event) => {
                    if event.kind != event::KeyEventKind::Press {
                        continue;
                    }

                    // Handle help overlay
                    if app_state.show_help {
                        if matches!(event.code, KeyCode::F(1) | KeyCode::Esc) {
                            app_state.show_help = false;
                        }
                        draw(&mut renderer, &app_state);
                        continue;
                    }

                    // Handle quit
                    if handle_quit(event) {
                        renderer.reset_terminal();
                        exit(0);
                    }

                    // Toggle help
                    if matches!(event.code, KeyCode::F(1)) {
                        app_state.show_help = !app_state.show_help;
                        draw(&mut renderer, &app_state);
                        continue;
                    }

                    // Handle favorites overlay
                    if favorites_active {
                        handle_favorites_input(
                            event,
                            &mut favorites_active,
                            &favorites_items,
                            &mut favorites_selected,
                            &fs_adapter,
                            &mut app_state,
                        );
                        draw_with_favorites(
                            &mut renderer,
                            &app_state,
                            favorites_active,
                            &favorites_items,
                            favorites_selected,
                        );
                        continue;
                    }

                    // Handle delete confirmation
                    if delete_paths.is_some() {
                        handle_delete_confirmation(
                            event,
                            &mut delete_paths,
                            &fs_adapter,
                            &mut app_state,
                        );
                        draw_with_delete(&mut renderer, &app_state, delete_paths.as_ref());
                        continue;
                    }

                    // Handle find mode
                    if find_filter.is_some() {
                        handle_find_input(
                            event,
                            &mut find_filter,
                            &fd_search,
                            &sender,
                            &fs_adapter,
                            &mut app_state,
                        );
                        draw_with_find(&mut renderer, &app_state, find_filter.as_ref());
                        continue;
                    }

                    // Handle ripgrep mode
                    if ripgrep_filter.is_some() {
                        handle_ripgrep_input(
                            event,
                            &mut ripgrep_filter,
                            &rg_search,
                            &fs_adapter,
                            &mut app_state,
                        );
                        draw_with_ripgrep(&mut renderer, &app_state, ripgrep_filter.as_ref());
                        continue;
                    }

                    // Switch pane in two-pane mode
                    if two_pane && event.code == KeyCode::Tab {
                        app_state.switch_pane();
                        draw(&mut renderer, &app_state);
                        continue;
                    }

                    // Feature shortcuts
                    let panel = app_state.active_panel_mut();

                    // Delete shortcut (Ctrl+W)
                    if event.code == KeyCode::Char('w')
                        && event.modifiers.contains(event::KeyModifiers::CONTROL)
                    {
                        let targets = if panel.multi_selected_count() > 0 {
                            panel.multi_selected_paths()
                        } else if let Some(path) = panel.get_selected_path() {
                            vec![path]
                        } else {
                            vec![]
                        };

                        if !targets.is_empty() {
                            delete_paths = Some(targets);
                        }
                        draw_with_delete(&mut renderer, &app_state, delete_paths.as_ref());
                        continue;
                    }

                    // Find shortcut (F6)
                    if event.code == KeyCode::F(6) {
                        find_filter = Some(String::new());
                        draw_with_find(&mut renderer, &app_state, find_filter.as_ref());
                        continue;
                    }

                    // RipGrep shortcut (F7)
                    if event.code == KeyCode::F(7) {
                        ripgrep_filter = Some(String::new());
                        draw_with_ripgrep(&mut renderer, &app_state, ripgrep_filter.as_ref());
                        continue;
                    }

                    // Favorites shortcut (Ctrl+D)
                    if event.code == KeyCode::Char('d')
                        && event.modifiers.contains(event::KeyModifiers::CONTROL)
                    {
                        if event.modifiers.contains(event::KeyModifiers::SHIFT) {
                            // Add current dir to favorites
                            let path = panel.current_path.to_string_lossy().to_string();
                            if !favorites_items.contains(&path) {
                                favorites_items.push(path);
                                save_favorites(&favorites_items);
                            }
                        }
                        favorites_active = true;
                        draw_with_favorites(
                            &mut renderer,
                            &app_state,
                            favorites_active,
                            &favorites_items,
                            favorites_selected,
                        );
                        continue;
                    }

                    // VS Code shortcut (F4)
                    if event.code == KeyCode::F(4) {
                        if let Some(path) = panel.get_selected_path() {
                            vscode_adapter.open(&path);
                        }
                        draw(&mut renderer, &app_state);
                        continue;
                    }

                    // Multi-select (Space)
                    if event.code == KeyCode::Char(' ')
                        && !matches!(panel.mode, PanelMode::QuickView(_))
                    {
                        panel.toggle_multi_selection(panel.cursor);
                        draw(&mut renderer, &app_state);
                        continue;
                    }

                    // Clear multi-selection (Esc when in normal/filter mode with selection)
                    if event.code == KeyCode::Esc
                        && panel.multi_selected_count() > 0
                        && !matches!(panel.mode, PanelMode::QuickView(_))
                    {
                        panel.clear_multi_selection();
                        draw(&mut renderer, &app_state);
                        continue;
                    }

                    // Handle based on mode
                    let panel = app_state.active_panel_mut();
                    match &panel.mode {
                        PanelMode::Normal => handle_normal_mode(
                            event,
                            &fs_adapter,
                            &open_adapter,
                            &mut clipboard_adapter,
                            &mut app_state,
                            &renderer,
                        ),
                        PanelMode::Filter => handle_filter_mode(
                            event,
                            &fs_adapter,
                            &open_adapter,
                            &mut clipboard_adapter,
                            &mut app_state,
                            &renderer,
                        ),
                        PanelMode::QuickView(_) => {
                            handle_quick_view_mode(event, &mut app_state, &renderer)
                        }
                    }

                    draw(&mut renderer, &app_state);
                }
                Event::Resize(cols, rows) => {
                    renderer.update_size(cols, rows);
                    draw(&mut renderer, &app_state);
                }
                _ => {}
            }
        }

        // Handle async messages
        if let Ok(message) = receiver.try_recv() {
            match message {
                Message::DrawFiles(files) => {
                    let panel = app_state.active_panel_mut();
                    panel.clear_notification_force();
                    let base_path = panel.current_path.clone();
                    navigate::replace_entries_from_search(panel, files, &base_path);
                    draw(&mut renderer, &app_state);
                }
            }
        }

        // Clear expired notifications
        let panel = app_state.active_panel_mut();
        panel.clear_notification_if_expired(3000);
    }
}

/// Draw the UI
fn draw(renderer: &mut TerminalRenderer, state: &AppState) {
    if state.two_pane_mode {
        renderer.draw_two_panes(state);
    } else {
        renderer.draw(state);
    }
}

/// Draw with delete confirmation overlay
fn draw_with_delete(
    renderer: &mut TerminalRenderer,
    state: &AppState,
    delete_paths: Option<&Vec<PathBuf>>,
) {
    draw(renderer, state);
    if let Some(paths) = delete_paths {
        draw_delete_prompt(renderer, paths);
    }
}

fn draw_delete_prompt(renderer: &mut TerminalRenderer, paths: &[PathBuf]) {
    use crossterm::style::Print;
    use crossterm::terminal::ClearType;
    use crossterm::{cursor, queue, terminal};

    let prompt = match paths {
        [] => "Delete: nothing selected".to_string(),
        [single] => format!("Delete: {}? [y/n]", single.display()),
        many => {
            let first = many.first().unwrap();
            format!(
                "Delete: {} (+{} more)? [y/n]",
                first.display(),
                many.len() - 1
            )
        }
    };

    let _ = queue!(
        &renderer.stdout,
        cursor::MoveTo(0, renderer.rows - FOOTER_ROWS + 1),
        terminal::Clear(ClearType::UntilNewLine),
        Print(prompt.red().bold()),
    );
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

/// Draw with find overlay
fn draw_with_find(renderer: &mut TerminalRenderer, state: &AppState, find_filter: Option<&String>) {
    draw(renderer, state);
    if let Some(filter) = find_filter {
        draw_find_prompt(renderer, filter);
    }
}

fn draw_find_prompt(renderer: &mut TerminalRenderer, filter: &str) {
    use crossterm::style::Print;
    use crossterm::terminal::ClearType;
    use crossterm::{cursor, queue, terminal};

    let _ = queue!(
        &renderer.stdout,
        cursor::MoveTo(0, renderer.rows - FOOTER_ROWS + 1),
        terminal::Clear(ClearType::UntilNewLine),
        Print(format!("Find: {} [enter/esc]", filter).magenta().bold()),
    );
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

/// Draw with ripgrep overlay
fn draw_with_ripgrep(
    renderer: &mut TerminalRenderer,
    state: &AppState,
    ripgrep_filter: Option<&String>,
) {
    draw(renderer, state);
    if let Some(filter) = ripgrep_filter {
        draw_ripgrep_prompt(renderer, filter);
    }
}

fn draw_ripgrep_prompt(renderer: &mut TerminalRenderer, filter: &str) {
    use crossterm::style::Print;
    use crossterm::terminal::ClearType;
    use crossterm::{cursor, queue, terminal};

    let _ = queue!(
        &renderer.stdout,
        cursor::MoveTo(0, renderer.rows - FOOTER_ROWS + 1),
        terminal::Clear(ClearType::UntilNewLine),
        Print(format!("RipGrep: {} [enter/esc]", filter).magenta().bold()),
    );
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

/// Draw with favorites overlay
fn draw_with_favorites(
    renderer: &mut TerminalRenderer,
    state: &AppState,
    active: bool,
    items: &[String],
    selected: usize,
) {
    if active {
        draw_favorites_overlay(renderer, items, selected);
    } else {
        draw(renderer, state);
    }
}

fn draw_favorites_overlay(renderer: &mut TerminalRenderer, items: &[String], selected: usize) {
    use crossterm::style::Print;
    use crossterm::terminal::ClearType;
    use crossterm::{cursor, queue, terminal};

    let rows_available = renderer.rows - HEADER_ROWS - FOOTER_ROWS;

    // Header
    let _ = queue!(
        &renderer.stdout,
        cursor::MoveTo(0, 0),
        Print("Favourites"),
        terminal::Clear(ClearType::UntilNewLine)
    );

    // Draw line
    let line = (0..renderer.columns).map(|_| "─").collect::<String>();
    let _ = queue!(
        &renderer.stdout,
        cursor::MoveTo(0, 1),
        Print(line.with(crossterm::style::Color::Blue))
    );

    // Content
    let _ = queue!(&renderer.stdout, cursor::MoveTo(0, HEADER_ROWS));

    for (i, item) in items.iter().enumerate() {
        let name = if i == selected {
            item.clone().dark_magenta().negative()
        } else {
            item.clone().dark_magenta()
        };

        let _ = queue!(
            &renderer.stdout,
            Print(name),
            terminal::Clear(ClearType::UntilNewLine),
            cursor::MoveToNextLine(1)
        );
    }

    let rows_to_clear: i16 = rows_available as i16 - items.len() as i16;
    if rows_to_clear > 0 {
        for _ in 0..rows_to_clear {
            let _ = queue!(
                &renderer.stdout,
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
    }

    let _ = std::io::Write::flush(&mut std::io::stdout());
}

fn handle_quit(event: KeyEvent) -> bool {
    (KeyCode::F(10) == event.code)
        || (KeyCode::Char('c') == event.code
            && event.modifiers.contains(event::KeyModifiers::CONTROL))
}

fn handle_normal_mode(
    event: KeyEvent,
    fs: &StdFileSystem,
    open_adapter: &SystemOpenAdapter,
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
        }
        KeyCode::Backspace => navigate::go_up_one_level(fs, panel),
        KeyCode::Up => navigate::move_cursor(panel, -1, visible_rows),
        KeyCode::Down => navigate::move_cursor(panel, 1, visible_rows),
        KeyCode::Home => navigate::navigate_home(panel),
        KeyCode::End => navigate::navigate_end(panel, visible_rows),
        KeyCode::Enter => {
            if event.modifiers.contains(event::KeyModifiers::CONTROL) {
                let absolute = event.modifiers.contains(event::KeyModifiers::SHIFT);
                let _ = file_ops::copy_to_clipboard(clipboard, panel, absolute);
            } else {
                // Try to enter directory
                if !navigate::enter_selected(fs, panel) {
                    // If not a directory, open file
                    if let Some(path) = panel.get_selected_path()
                        && fs.is_file(&path)
                    {
                        open_adapter.open(&path);
                    }
                }
            }
        }
        KeyCode::F(3) => open_quick_view(panel, renderer.columns),
        _ => {}
    }
}

fn handle_filter_mode(
    event: KeyEvent,
    fs: &StdFileSystem,
    open_adapter: &SystemOpenAdapter,
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
        KeyCode::Enter => {
            if event.modifiers.contains(event::KeyModifiers::CONTROL) {
                let absolute = event.modifiers.contains(event::KeyModifiers::SHIFT);
                let _ = file_ops::copy_to_clipboard(clipboard, panel, absolute);
            } else {
                // Try to enter directory
                if !navigate::enter_selected(fs, panel) {
                    // If not a directory, open file
                    if let Some(path) = panel.get_selected_path()
                        && fs.is_file(&path)
                    {
                        open_adapter.open(&path);
                    }
                }
            }
        }
        KeyCode::F(3) => open_quick_view(panel, renderer.columns),
        _ => {}
    }
}

fn handle_quick_view_mode(event: KeyEvent, state: &mut AppState, renderer: &TerminalRenderer) {
    let panel = state.active_panel_mut();
    let visible_rows = renderer.visible_rows();

    match event.code {
        KeyCode::Up => scroll_content(panel, -1, renderer.rows),
        KeyCode::Down => scroll_content(panel, 1, renderer.rows),
        KeyCode::PageUp => scroll_content(panel, -(renderer.rows as isize).max(1), renderer.rows),
        KeyCode::PageDown => scroll_content(panel, renderer.rows as isize, renderer.rows),
        KeyCode::Left => {
            navigate::move_cursor(panel, -1, visible_rows);
            open_quick_view(panel, renderer.columns);
        }
        KeyCode::Right => {
            navigate::move_cursor(panel, 1, visible_rows);
            open_quick_view(panel, renderer.columns);
        }
        KeyCode::Esc | KeyCode::F(3) => {
            panel.mode = PanelMode::Normal;
        }
        _ => {}
    }
}

fn scroll_content(panel: &mut application::PanelState, direction: isize, rows: u16) {
    if let PanelMode::QuickView(QuickViewMode::Text {
        lines,
        start,
        length,
    }) = &panel.mode
    {
        let visible_rows = rows.saturating_sub(HEADER_ROWS + FOOTER_ROWS) as usize;
        let max_start = length.saturating_sub(visible_rows.max(1));
        let new_start = (*start as isize + direction).clamp(0, max_start as isize) as usize;

        panel.mode = PanelMode::QuickView(QuickViewMode::Text {
            lines: lines.clone(),
            start: new_start,
            length: *length,
        });
    }
}

fn handle_delete_confirmation(
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
            KeyCode::Char('n') | KeyCode::Esc => {
                *delete_paths = None;
            }
            _ => {}
        }
    }
}

fn handle_find_input(
    event: KeyEvent,
    find_filter: &mut Option<String>,
    _search: &FdSearchAdapter,
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
                let sender = sender.clone();
                let pwd = state.active_panel().current_path.clone();
                let query = filter.clone();
                state
                    .active_panel_mut()
                    .set_notification("Searching...".to_string());

                thread::spawn(move || {
                    let results = FdSearchAdapter::new().find(&query, &pwd);
                    let _ = sender.send(Message::DrawFiles(results));
                });

                *find_filter = None;
            }
            KeyCode::Esc => {
                *find_filter = None;
                let panel = state.active_panel_mut();
                navigate::refresh_entries(fs, panel);
            }
            _ => {}
        }
    }
}

fn handle_ripgrep_input(
    event: KeyEvent,
    ripgrep_filter: &mut Option<String>,
    _search: &RipGrepAdapter,
    fs: &StdFileSystem,
    state: &mut AppState,
) {
    if let Some(filter) = ripgrep_filter {
        match event.code {
            KeyCode::Char(c) => filter.push(c),
            KeyCode::Backspace => {
                filter.pop();
            }
            KeyCode::Enter => {
                let results =
                    RipGrepAdapter::new().find(filter, &state.active_panel().current_path);
                let panel = state.active_panel_mut();
                let base_path = panel.current_path.clone();
                navigate::replace_entries_from_search(panel, results, &base_path);
                *ripgrep_filter = None;
            }
            KeyCode::Esc => {
                *ripgrep_filter = None;
                let panel = state.active_panel_mut();
                navigate::refresh_entries(fs, panel);
            }
            _ => {}
        }
    }
}

fn handle_favorites_input(
    event: KeyEvent,
    favorites_active: &mut bool,
    favorites_items: &[String],
    favorites_selected: &mut usize,
    fs: &StdFileSystem,
    state: &mut AppState,
) {
    match event.code {
        KeyCode::Up => {
            *favorites_selected = favorites_selected.saturating_sub(1);
        }
        KeyCode::Down => {
            *favorites_selected =
                (*favorites_selected + 1).min(favorites_items.len().saturating_sub(1));
        }
        KeyCode::Enter => {
            if let Some(item) = favorites_items.get(*favorites_selected) {
                let path = PathBuf::from(item);
                let panel = state.active_panel_mut();
                navigate::change_directory(fs, panel, path);
                *favorites_active = false;
            }
        }
        KeyCode::Esc => {
            *favorites_active = false;
        }
        _ => {}
    }
}

fn load_favorites() -> Vec<String> {
    std::fs::read_to_string("favorites.txt")
        .unwrap_or_default()
        .lines()
        .map(|s| s.to_string())
        .collect()
}

fn save_favorites(items: &[String]) {
    let _ = std::fs::write("favorites.txt", items.join("\n"));
}

// Quick view implementation (moved from files_view.rs)

const MIN_WRAP_WIDTH: u16 = 20;
const DIRECTORY_PREVIEW_LIMIT: usize = 20;

fn open_quick_view(panel: &mut application::PanelState, wrap_width: u16) {
    if panel.entries.is_empty() || panel.cursor >= panel.entries.len() {
        return;
    }

    let entry = &panel.entries[panel.cursor];
    let file_path = entry.path.clone();
    let selected_file = entry.name.clone();

    show_file_quick_view(panel, file_path, &selected_file, wrap_width);
}

fn show_file_quick_view(
    panel: &mut application::PanelState,
    file_path: PathBuf,
    _selected_file: &str,
    wrap_width: u16,
) {
    let meta = fs::metadata(&file_path).is_ok();

    if !meta {
        panel.mode = PanelMode::QuickView(QuickViewMode::NotSupported);
        return;
    }

    if file_path.is_dir() {
        show_directory_quick_view(panel, &file_path);
        return;
    }

    let ftype = get_type_from_path(&file_path);
    match ftype {
        FileType::Text => show_text_quick_view(panel, &file_path, wrap_width),
        FileType::Image => show_image_quick_view(panel, &file_path),
        FileType::Other => {
            panel.mode = PanelMode::QuickView(QuickViewMode::NotSupported);
        }
    }
}

enum FileType {
    Text,
    Image,
    Other,
}

fn get_type_from_path(file_path: &Path) -> FileType {
    if let Some(mime_type) = tree_magic_mini::from_filepath(file_path) {
        match mime_type.split('/').next() {
            Some("text") => FileType::Text,
            Some("image") => FileType::Image,
            _ => match file_path.extension().and_then(std::ffi::OsStr::to_str) {
                Some("txt") | Some("md") | Some("rs") | Some("toml") => FileType::Text,
                Some("png") | Some("jpg") | Some("jpeg") | Some("gif") => FileType::Image,
                _ => FileType::Other,
            },
        }
    } else {
        FileType::Other
    }
}

fn show_text_quick_view(panel: &mut application::PanelState, file_path: &Path, wrap_width: u16) {
    if let Ok(content) = fs::read_to_string(file_path) {
        let width = wrap_width.saturating_sub(4).max(MIN_WRAP_WIDTH) as usize;
        let lines: Vec<String> = textwrap::wrap(&content, width)
            .into_iter()
            .map(|line| line.to_string())
            .collect();
        let length = lines.len();
        panel.mode = PanelMode::QuickView(QuickViewMode::Text {
            lines: syntax_highlight_text(lines),
            start: 0,
            length,
        });
    }
}

fn syntax_highlight_text(lines: Vec<String>) -> Vec<String> {
    use crossterm::style::Stylize;

    let comment_markers = ["//", "#", "--"];
    let keyword_markers = vec![
        "fn", "let", "if", "else", "for", "while", "match", "struct", "enum", "impl", "function",
        "trait", "mod", "pub", "private", "self", "super", "const", "var", "static", "type",
        "async", "await", "return", "break", "continue", "match", "loop", "in", "as", "where",
        "crate", "extern", "dyn", "ref", "mut",
    ];
    let keywords_fullline = ["derive", "use", "import"];

    lines
        .iter()
        .map(|line| {
            let mut comment_started = false;
            let mut fullline_keyword_started = false;
            line.split(' ')
                .map(|word| {
                    if comment_started
                        || comment_markers
                            .iter()
                            .any(|marker| word.starts_with(marker))
                    {
                        comment_started = true;
                        format!("{} ", word.with(crossterm::style::Color::DarkGreen))
                    } else if keyword_markers.contains(&word) {
                        format!("{} ", word.with(crossterm::style::Color::Yellow))
                    } else if fullline_keyword_started || keywords_fullline.contains(&word) {
                        fullline_keyword_started = true;
                        format!("{} ", word.with(crossterm::style::Color::Cyan))
                    } else {
                        word.to_string()
                    }
                })
                .collect::<Vec<String>>()
                .join(" ")
        })
        .collect()
}

fn show_directory_quick_view(panel: &mut application::PanelState, file_path: &Path) {
    let mut files = vec![];
    let mut dirs = vec![];
    let mut total_size: u64 = 0;
    let mut total_entries = 0usize;

    let entries = fs::read_dir(file_path);
    if let Ok(entries) = entries {
        for entry in entries.flatten() {
            total_entries += 1;
            let file_name = entry.file_name().into_string().unwrap_or_default();
            let meta = entry.metadata().ok();
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                dirs.push(file_name);
            } else {
                if let Some(len) = meta.as_ref().map(|m| m.len()) {
                    total_size += len;
                }
                files.push(file_name);
            }
        }
    }

    dirs.sort();
    files.sort();

    let mut lines = vec![];
    lines.push(format!("Directory: {}", file_path.display()));
    lines.push(format!(
        "Entries: {} (dirs: {}, files: {})",
        total_entries,
        dirs.len(),
        files.len()
    ));
    lines.push(format!(
        "Size (files only): {}",
        human_readable_size(total_size)
    ));
    lines.push(String::new());

    if !dirs.is_empty() {
        lines.push("Directories:".into());
        for dir in dirs.iter().take(DIRECTORY_PREVIEW_LIMIT) {
            lines.push(format!("{}/", dir));
        }
        if dirs.len() > DIRECTORY_PREVIEW_LIMIT {
            lines.push(format!(
                "... and {} more",
                dirs.len() - DIRECTORY_PREVIEW_LIMIT
            ));
        }
        lines.push(String::new());
    }

    if !files.is_empty() {
        lines.push("Files:".into());
        for file in files.iter().take(DIRECTORY_PREVIEW_LIMIT) {
            lines.push(file.to_string());
        }
        if files.len() > DIRECTORY_PREVIEW_LIMIT {
            lines.push(format!(
                "... and {} more",
                files.len() - DIRECTORY_PREVIEW_LIMIT
            ));
        }
    }

    panel.mode = PanelMode::QuickView(QuickViewMode::Directory { lines });
}

fn show_image_quick_view(panel: &mut application::PanelState, file_path: &Path) {
    let now = Instant::now();
    let thumb = extract_embedded_thumbnail(file_path);
    let image_pixels = if let Some(thumb) = thumb {
        Some(thumb.to_rgb8())
    } else {
        match image::open(file_path) {
            Ok(img) => Some(img.to_rgb8()),
            _ => None,
        }
    };

    let mut f = match File::open(file_path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return;
    }
    logger::log(&format!("Image loading took: {:?}", now.elapsed()));

    if let Some(image_pixels) = image_pixels {
        let pixels_vec: Vec<u8> = image_pixels.into_raw();
        panel.mode = PanelMode::QuickView(QuickViewMode::Image(pixels_vec, buf));
    }
}

fn extract_embedded_thumbnail(path: &Path) -> Option<DynamicImage> {
    let metadata = Metadata::new_from_path(path).ok()?;

    let thumb_offset = metadata
        .get_tag(&ExifTag::ThumbnailOffset(vec![], vec![]))
        .next()?;

    let thumb_data = if let ExifTag::ThumbnailOffset(_, data) = thumb_offset {
        data
    } else {
        return None;
    };

    let img = load_from_memory(thumb_data).ok()?;
    Some(img)
}

fn human_readable_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.2} {}", size, UNITS[unit])
    }
}
