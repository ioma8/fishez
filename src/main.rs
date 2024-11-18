mod files_view;
mod logger;
mod terminal_ui;
use crossterm::{
    cursor, event::{self, Event, KeyCode, KeyEvent}, execute, queue, style::{Print, Stylize}, terminal::{self, disable_raw_mode, enable_raw_mode, ClearType}
};
use files_view::{FilesView, FilesViewMode, FOOTER_ROWS, HEADER_ROWS};
use std::io::stdout;
use std::panic;
use terminal_ui::{FeatureTrait, TerminalUI};

fn main() {
    setup_terminal();
    let mut ui = TerminalUI::new();
    ui.add_feature(Box::new(FavouritesFeature::new()));

    let mut files_view = FilesView::new();
    files_view.update();

    enable_raw_mode().expect("Failed to enable raw mode");

    // TODO: favorites - oblibene polozky
    // TODO: nejak pridat zoxide?

    loop {
        files_view.clear_notification();

        ui.draw_ui(&files_view);

        match event::read().unwrap() {
            Event::Key(event) => handle_key_event(&mut files_view, event, &mut ui),
            Event::Resize(cols, rows) => {
                ui.columns = cols;
                ui.rows = rows;
            }
            _ => {}
        }
    }
}

struct FavouritesFeature {
    items: Vec<String>,
    selected_index: usize,
}

impl FavouritesFeature {
    fn new() -> Self {
        let items = std::fs::read_to_string("favorites.txt")
            .unwrap_or_else(|_| String::new())
            .lines()
            .map(|line| line.to_string())
            .collect();
        Self {
            items: items,
            selected_index: 0,
        }
    }
}

impl FeatureTrait for FavouritesFeature {
    fn get_id(&self) -> &'static str {
        "favourites"
    }

    fn general_shortcuts(&mut self, event: KeyEvent, files_view: &mut FilesView) -> bool {
        if event.code == KeyCode::Char('d')
            && event.modifiers.contains(event::KeyModifiers::CONTROL)
        {
            files_view.feature_active = Some(self.get_id().to_string());
            return true;
        }
        return false;
    }

    fn view_shortcuts(&mut self, event: KeyEvent, files_view: &mut FilesView) {
        match event.code {
            KeyCode::Up => self.selected_index = self.selected_index.saturating_sub(1),
            KeyCode::Down => self.selected_index = self.selected_index.saturating_add(1),
            KeyCode::Enter => {
                if let Some(item) = self.items.get(self.selected_index) {
                    files_view.pwd = item.clone();
                    files_view.update();
                    files_view.feature_active = None;
                }
            }
            KeyCode::Esc => files_view.feature_active = None,
            _ => {}
        }
    }

    fn draw_content(&self, files_view: &FilesView, terminal_ui: &TerminalUI) {
        let rows_available = terminal_ui.rows - HEADER_ROWS - FOOTER_ROWS;
        // TODO: add scroll of items if they dont fit on screen

        let _ = queue!(&terminal_ui.stdout, cursor::MoveTo(0, HEADER_ROWS));

        for (i, item) in self.items.iter().enumerate() {
            let name = if (i == self.selected_index) {
                item.clone().dark_magenta().negative()
            } else {
                item.clone().dark_magenta()
            };

            let _ = queue!(
                &terminal_ui.stdout,
                Print(name),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }

        let rows_to_clear: i16 = rows_available as i16 - self.items.len() as i16;

        if rows_to_clear > 0 {
            for _ in 0..rows_to_clear {
                let _ = queue!(
                    &terminal_ui.stdout,
                    terminal::Clear(ClearType::UntilNewLine),
                    cursor::MoveToNextLine(1)
                );
            }
        }
    }
    
    fn draw_header(&self, files_view: &FilesView, terminal_ui: &TerminalUI) {
        let _ = queue!(&terminal_ui.stdout, cursor::MoveTo(0, 0), Print("Favourites"));
        // TODO: better drawing of the header - the right part (FISHEZ / NOTIFICATION)
    }
    
    fn draw_footer(&self, files_view: &FilesView, terminal_ui: &TerminalUI) {
        todo!()
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
    disable_raw_mode().expect("Failed to disable raw mode");
}

fn handle_key_event(files_view: &mut FilesView, event: KeyEvent, ui: &mut TerminalUI) {
    if event.kind != event::KeyEventKind::Press {
        return;
    }

    if handle_quit(event) {
        return;
    }

    if ui.handle_features_shortcuts(event, files_view) {
        return;
    }
    

    match &files_view.mode {
        FilesViewMode::Normal => handle_normal_mode(files_view, event, ui.rows),
        FilesViewMode::Filter => handle_filter_mode(files_view, event, ui.rows),
        FilesViewMode::QuickView(_) => handle_quick_view_mode(files_view, event, ui.rows),
        FilesViewMode::RecursiveSearch => handle_recursive_search_mode(files_view, event, ui.rows),
        FilesViewMode::RipGrep => handle_ripgrep_mode(files_view, event, ui.rows),
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
        _ => {}
    }
}

fn handle_quit(event: KeyEvent) -> bool {
    if (KeyCode::F(10) == event.code)
        || (KeyCode::Char('c') == event.code
            && event.modifiers.contains(event::KeyModifiers::CONTROL))
    {
        std::thread::spawn(|| {
            reset_terminal();
            std::process::exit(0);
        });
        return true;
    } else {
        return false;
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
        // TODO: mkdir shortcut
        // TODO: touch shortcut
        _ => handle_normal_navigation(files_view, event, rows),
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
        _ => handle_normal_navigation(files_view, event, rows),
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
        _ => {}
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
