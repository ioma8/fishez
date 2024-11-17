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
        FilesViewMode::Normal => handle_normal_mode(files_view, event, rows),
        FilesViewMode::Filter => handle_filter_mode(files_view, event, rows),
        FilesViewMode::QuickView(_) => handle_quick_view_mode(files_view, event, rows),
        FilesViewMode::RecursiveSearch => handle_recursive_search_mode(files_view, event, rows),
        FilesViewMode::RipGrep => handle_ripgrep_mode(files_view, event, rows),
    }
}

fn handle_normal_navigation(files_view: &mut FilesView, event: KeyEvent, rows: u16) {
    match event.code {
        KeyCode::Up => files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Down => files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Home => files_view.navigate_home(),
        KeyCode::End => files_view.navigate_end(rows - HEADER_ROWS - FOOTER_ROWS),
        KeyCode::Enter => {
            if event.modifiers.contains(event::KeyModifiers::CONTROL) {
                if event.modifiers.contains(event::KeyModifiers::SHIFT) {
                    files_view.copy_selected_to_clipboard(true);
                } else {
                    files_view.copy_selected_to_clipboard(false);
                }
            } else {
                files_view.open_selected_file();
            }
        }
        KeyCode::F(3) => files_view.open_quick_view(),
        KeyCode::F(4) => files_view.open_in_editor(),
        rest => handle_quit(files_view, rest),
    }
}

fn handle_quit(files_view: &mut FilesView, code: KeyCode) {
    match code {
        KeyCode::F(10) => {
            reset_terminal();
            std::process::exit(0);
        }
        _ => {}
    }
}

fn handle_normal_mode(files_view: &mut FilesView, event: KeyEvent, rows: u16) {
    match event.code {
        KeyCode::Char(char) => {
            files_view.mode = FilesViewMode::Filter;
            files_view.filter_string.push(char);
        }
        KeyCode::Backspace => files_view.go_up_one_level(),
        KeyCode::F(6) => files_view.mode = FilesViewMode::RecursiveSearch,
        KeyCode::F(7) => files_view.mode = FilesViewMode::RipGrep,
        rest => handle_normal_navigation(files_view, event, rows),
    }
}

fn handle_filter_mode(files_view: &mut FilesView, event: KeyEvent, rows: u16) {
    match event.code {
        KeyCode::Esc => files_view.reset_filter_mode(),
        KeyCode::Backspace => files_view.update_filter_string(|s| {
            s.pop();
        }),
        KeyCode::Char(c) => files_view.update_filter_string(|s| {
            s.push(c);
        }),
        rest => handle_normal_navigation(files_view, event, rows),
    }
}

fn handle_quick_view_mode(files_view: &mut FilesView, event: KeyEvent, rows: u16) {
    match event.code {
        KeyCode::Up => files_view.scroll_content(-1),
        KeyCode::Down => files_view.scroll_content(1),
        KeyCode::PageUp => files_view.scroll_content(-10),
        KeyCode::PageDown => files_view.scroll_content(10),
        KeyCode::Left => {
            files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS);
            files_view.open_quick_view();
        }
        KeyCode::Right => {
            files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS);
            files_view.open_quick_view();
        }
        KeyCode::Esc | KeyCode::F(3) => files_view.mode = FilesViewMode::Normal,
        rest => handle_quit(files_view, rest),
    }
}

fn handle_recursive_search_mode(files_view: &mut FilesView, event: KeyEvent, rows: u16) {
    match event.code {
        KeyCode::Esc => files_view.reset_filter_mode(),
        KeyCode::Char(c) => {
            files_view.filter_string.push(c);
            files_view.perform_recursive_search();
        }
        KeyCode::Backspace => {
            files_view.filter_string.pop();
            files_view.perform_recursive_search();
        }
        _ => handle_normal_navigation(files_view, event, rows),
    }
}

fn handle_ripgrep_mode(files_view: &mut FilesView, event: KeyEvent, rows: u16) {
    match event.code {
        KeyCode::Esc => files_view.reset_filter_mode(),
        KeyCode::Char(c) => {
            files_view.filter_string.push(c);
            files_view.perform_ripgrep_search();
        }
        KeyCode::Backspace => {
            files_view.filter_string.pop();
            files_view.perform_ripgrep_search();
        }
        _ => handle_normal_navigation(files_view, event, rows),
    }
}
