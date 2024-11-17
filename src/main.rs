mod files_view;
mod logger;
mod terminal_ui;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute, terminal,
};
use files_view::{FilesView, FilesViewMode, FOOTER_ROWS, HEADER_ROWS};
use std::io::stdout;
use std::panic;
use terminal_ui::TerminalUI;

fn main() {
    setup_terminal();
    let mut ui = TerminalUI::new();
    let mut files_view = FilesView::new();
    files_view.update();

    // TODO: favorites - oblibene polozky
    // TODO: nejak pridat zoxide?

    loop {
        ui.draw_ui(&files_view);

        match event::read().unwrap() {
            Event::Key(event) => handle_key_event(&mut files_view, event, ui.rows),
            Event::Resize(cols, rows) => {
                ui.columns = cols;
                ui.rows = rows;
            }
            _ => {}
        }
    }
}

fn setup_terminal() {
    ctrlc::set_handler(|| {
        reset_terminal();
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    panic::set_hook(Box::new(|_| {
        reset_terminal();
    }));
}

fn reset_terminal() {
    let _ = execute!(
        stdout(),
        cursor::MoveTo(0, 0),
        cursor::Show,
        cursor::EnableBlinking,
        terminal::Clear(terminal::ClearType::All)
    );
}

fn handle_key_event(files_view: &mut FilesView, event: KeyEvent, rows: u16) {
    if event.kind != event::KeyEventKind::Press {
        return;
    }

    match files_view.mode {
        FilesViewMode::Normal => handle_normal_mode(files_view, event.code, rows),
        FilesViewMode::Filter => handle_filter_mode(files_view, event.code, rows),
        FilesViewMode::QuickView(_) => handle_quick_view_mode(files_view, event.code),
        FilesViewMode::RecursiveSearch => {
            handle_recursive_search_mode(files_view, event.code, rows)
        }
        FilesViewMode::RipGrep => handle_ripgrep_mode(files_view, event.code, rows),
    }
}

fn handle_normal_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Char('s') => files_view.mode = FilesViewMode::Filter,
        KeyCode::Char('f') => files_view.mode = FilesViewMode::RecursiveSearch,
        KeyCode::Char('r') => files_view.mode = FilesViewMode::RipGrep,
        KeyCode::F(3) => files_view.toggle_quick_view(),
        KeyCode::F(4) => files_view.open_in_editor(),
        KeyCode::Up => files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Down => files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Home => files_view.navigate_home(),
        KeyCode::End => files_view.navigate_end(rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Backspace => files_view.go_up_one_level(),
        KeyCode::Enter => files_view.open_selected_file(),
        KeyCode::Char('q') => {
            reset_terminal();
            std::process::exit(0);
        }
        _ => {}
    }
}

fn handle_filter_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Esc => files_view.reset_filter_mode(),
        KeyCode::Backspace => files_view.update_filter_string(|s| {
            s.pop();
        }),
        KeyCode::Char(c) => files_view.update_filter_string(|s| {
            s.push(c);
        }),
        KeyCode::Up => files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Down => files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Home => files_view.navigate_home(),
        KeyCode::End => files_view.navigate_end(rows - HEADER_ROWS - FOOTER_ROWS),
        _ => {}
    }
}

fn handle_quick_view_mode(files_view: &mut FilesView, code: KeyCode) {
    match code {
        KeyCode::Up => files_view.scroll_content(-1),
        KeyCode::Down => files_view.scroll_content(1),
        KeyCode::Esc | KeyCode::F(3) => files_view.mode = FilesViewMode::Normal,
        _ => {}
    }
}

fn handle_recursive_search_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Esc => files_view.reset_filter_mode(),
        KeyCode::Char(c) => {
            files_view.filter_string.push(c);
            files_view.perform_recursive_search();
        }
        KeyCode::Backspace => {
            files_view.filter_string.pop();
            files_view.perform_recursive_search();
        }
        KeyCode::Up => files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Down => files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Home => files_view.navigate_home(),
        KeyCode::End => files_view.navigate_end(rows - HEADER_ROWS - FOOTER_ROWS),
        _ => {}
    }
}

fn handle_ripgrep_mode(files_view: &mut FilesView, code: KeyCode, rows: u16) {
    match code {
        KeyCode::Esc => files_view.reset_filter_mode(),
        KeyCode::Char(c) => {
            files_view.filter_string.push(c);
            files_view.perform_ripgrep_search();
        }
        KeyCode::Backspace => {
            files_view.filter_string.pop();
            files_view.perform_ripgrep_search();
        }
        KeyCode::Up => files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Down => files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Home => files_view.navigate_home(),
        KeyCode::End => files_view.navigate_end(rows - HEADER_ROWS - FOOTER_ROWS),
        _ => {}
    }
}
