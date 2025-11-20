mod features;
mod files_view;
mod logger;
mod terminal_ui;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use features::{
    delete_feature::DeleteFeature, favorites_feature::FavouritesFeature, find_feature::FindFeature,
    multiselect_feature::MultiSelectFeature, open_feature::OpenFeature,
    ripgrep_feature::RipGrepFeature, vscode_feature::VsCodeFeature,
};
use files_view::{FilesView, FilesViewMode, FOOTER_ROWS, HEADER_ROWS};
use std::{process::exit, sync::mpsc};
use terminal_ui::Message;
use terminal_ui::TerminalUI;
use terminal_ui::ActivePane;
use std::env;

fn main() {
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

    let two_pane = env::args().any(|arg| arg == "--two-pane" || arg == "-2");
    let (sender, receiver) = mpsc::channel::<Message>();
    let mut ui = TerminalUI::new(sender.clone());
    ui.add_feature(Box::new(VsCodeFeature::new()));
    ui.add_feature(Box::new(FindFeature::new()));
    ui.add_feature(Box::new(RipGrepFeature::new()));
    ui.add_feature(Box::new(FavouritesFeature::new()));
    ui.add_feature(Box::new(DeleteFeature::new()));
    ui.add_feature(Box::new(OpenFeature::new()));
    ui.add_feature(Box::new(MultiSelectFeature::new()));

    let mut left_files_view = FilesView::new();
    left_files_view.update();
    let mut right_files_view = FilesView::new();
    right_files_view.update();
    let mut active_pane = ActivePane::Left;

    // TODO: nejak pridat zoxide?

    if two_pane {
        ui.draw_ui_two_panes(&mut left_files_view, &mut right_files_view, active_pane);
    } else {
        ui.draw_ui(&mut left_files_view);
    }

    loop {
        if event::poll(std::time::Duration::from_millis(100)).unwrap() {
            match event::read().unwrap() {
                Event::Key(event) => {
                    if two_pane {
                        handle_key_event_two_panes(
                            &mut left_files_view,
                            &mut right_files_view,
                            &mut active_pane,
                            event,
                            &mut ui,
                        );
                        ui.draw_ui_two_panes(
                            &mut left_files_view,
                            &mut right_files_view,
                            active_pane,
                        );
                    } else {
                        handle_key_event_single(&mut left_files_view, event, &mut ui);
                        ui.draw_ui(&mut left_files_view);
                    }
                }
                Event::Resize(cols, rows) => {
                    ui.columns = cols;
                    ui.rows = rows;
                    if two_pane {
                        ui.draw_ui_two_panes(
                            &mut left_files_view,
                            &mut right_files_view,
                            active_pane,
                        );
                    } else {
                        ui.draw_ui(&mut left_files_view);
                    }
                }
                _ => {}
            }
        }

        if let Ok(message) = receiver.try_recv() {
            match message {
                Message::DrawFiles(files) => {
                    if two_pane {
                        let active_view = match active_pane {
                            ActivePane::Left => &mut left_files_view,
                            ActivePane::Right => &mut right_files_view,
                        };
                        active_view.clear_notification_force();
                        active_view.replace_files(files);
                        ui.draw_ui_two_panes(
                            &mut left_files_view,
                            &mut right_files_view,
                            active_pane,
                        );
                    } else {
                        left_files_view.clear_notification_force();
                        left_files_view.replace_files(files);
                        ui.draw_ui(&mut left_files_view);
                    }
                }
            }
        }
    }
}

fn handle_key_event_two_panes(
    left: &mut FilesView,
    right: &mut FilesView,
    active_pane: &mut ActivePane,
    event: KeyEvent,
    ui: &mut TerminalUI,
) {
    if event.kind != event::KeyEventKind::Press {
        return;
    }

    if ui.show_help {
        if matches!(event.code, KeyCode::F(1) | KeyCode::Esc) {
            ui.show_help = false;
        }
        return;
    }

    if handle_quit(event) {
        ui.reset_terminal();
        exit(0);
    }

    if matches!(event.code, KeyCode::F(1)) {
        ui.show_help = !ui.show_help;
        return;
    }

    if event.code == KeyCode::Tab {
        *active_pane = match active_pane {
            ActivePane::Left => ActivePane::Right,
            ActivePane::Right => ActivePane::Left,
        };
        return;
    }

    let active_view = match active_pane {
        ActivePane::Left => left,
        ActivePane::Right => right,
    };

    if ui.handle_features_shortcuts(event, active_view) {
        return;
    }

    match &active_view.mode {
        FilesViewMode::Normal => handle_normal_mode(active_view, event, ui.rows, ui.columns),
        FilesViewMode::Filter => handle_filter_mode(active_view, event, ui.rows, ui.columns),
        FilesViewMode::QuickView(_) => handle_quick_view_mode(active_view, event, ui.rows, ui.columns),
    }
}

fn handle_key_event_single(files_view: &mut FilesView, event: KeyEvent, ui: &mut TerminalUI) {
    if event.kind != event::KeyEventKind::Press {
        return;
    }

    if ui.show_help {
        if matches!(event.code, KeyCode::F(1) | KeyCode::Esc) {
            ui.show_help = false;
        }
        return;
    }

    if handle_quit(event) {
        ui.reset_terminal();
        exit(0);
    }

    if matches!(event.code, KeyCode::F(1)) {
        ui.show_help = !ui.show_help;
        return;
    }

    if ui.handle_features_shortcuts(event, files_view) {
        return;
    }

    match &files_view.mode {
        FilesViewMode::Normal => handle_normal_mode(files_view, event, ui.rows, ui.columns),
        FilesViewMode::Filter => handle_filter_mode(files_view, event, ui.rows, ui.columns),
        FilesViewMode::QuickView(_) => handle_quick_view_mode(files_view, event, ui.rows, ui.columns),
    }
}

fn handle_normal_navigation(files_view: &mut FilesView, event: KeyEvent, rows: u16, cols: u16) {
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
        KeyCode::F(3) => files_view.open_quick_view(cols),
        _ => {}
    }
}

fn handle_quit(event: KeyEvent) -> bool {
    if (KeyCode::F(10) == event.code)
        || (KeyCode::Char('c') == event.code
            && event.modifiers.contains(event::KeyModifiers::CONTROL))
    {
        true
    } else {
        false
    }
}

fn handle_normal_mode(files_view: &mut FilesView, event: KeyEvent, rows: u16, cols: u16) {
    match event.code {
        KeyCode::Char(char) => {
            files_view.mode = FilesViewMode::Filter;
            files_view.filter_string.push(char);
        }
        KeyCode::Backspace => files_view.go_up_one_level(),
        // TODO: mkdir shortcut
        // TODO: touch shortcut
        _ => handle_normal_navigation(files_view, event, rows, cols),
    }
}

fn handle_filter_mode(files_view: &mut FilesView, event: KeyEvent, rows: u16, cols: u16) {
    match event.code {
        KeyCode::Esc => files_view.reset_filter_mode(),
        KeyCode::Backspace => files_view.update_filter_string(|s| {
            s.pop();
        }),
        KeyCode::Char(c) => files_view.update_filter_string(|s| {
            s.push(c);
        }),
        _ => handle_normal_navigation(files_view, event, rows, cols),
    }
}

fn handle_quick_view_mode(files_view: &mut FilesView, event: KeyEvent, rows: u16, cols: u16) {
    match event.code {
        KeyCode::Up => files_view.scroll_content(-1, rows),
        KeyCode::Down => files_view.scroll_content(1, rows),
        KeyCode::PageUp => files_view.scroll_content(-(rows as isize).max(1), rows),
        KeyCode::PageDown => files_view.scroll_content(rows as isize, rows),
        KeyCode::Left => {
            files_view.navigate(-1, rows - HEADER_ROWS - FOOTER_ROWS);
            files_view.open_quick_view(cols);
        }
        KeyCode::Right => {
            files_view.navigate(1, rows - HEADER_ROWS - FOOTER_ROWS);
            files_view.open_quick_view(cols);
        }
        KeyCode::Esc | KeyCode::F(3) => files_view.mode = FilesViewMode::Normal,
        _ => {}
    }
}
