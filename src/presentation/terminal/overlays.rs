//! Terminal overlays - modal prompts and dialogs.

use crate::application::AppState;
use crate::presentation::{FOOTER_ROWS, HEADER_ROWS, TerminalRenderer};
use crossterm::style::{Color, Print, Stylize};
use crossterm::terminal::ClearType;
use crossterm::{cursor, queue, terminal};
use std::io::Write;
use std::path::PathBuf;

/// Draw with delete confirmation overlay.
pub fn draw_with_delete(
    renderer: &mut TerminalRenderer,
    state: &AppState,
    paths: Option<&Vec<PathBuf>>,
) {
    draw_main(renderer, state);
    if let Some(p) = paths {
        draw_delete_prompt(renderer, p);
    }
}

fn draw_delete_prompt(renderer: &mut TerminalRenderer, paths: &[PathBuf]) {
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
    let prompt_row = renderer.rows - FOOTER_ROWS + 1;
    let _ = queue!(
        renderer.writer(),
        cursor::MoveTo(0, prompt_row),
        terminal::Clear(ClearType::UntilNewLine),
        Print(prompt.red().bold()),
    );
    let _ = std::io::stdout().flush();
}

/// Draw with find prompt overlay.
pub fn draw_with_find(renderer: &mut TerminalRenderer, state: &AppState, filter: Option<&String>) {
    draw_main(renderer, state);
    if let Some(f) = filter {
        draw_find_prompt(renderer, f);
    }
}

fn draw_find_prompt(renderer: &mut TerminalRenderer, filter: &str) {
    let prompt_row = renderer.rows - FOOTER_ROWS + 1;
    let _ = queue!(
        renderer.writer(),
        cursor::MoveTo(0, prompt_row),
        terminal::Clear(ClearType::UntilNewLine),
        Print(format!("Find: {} [enter/esc]", filter).magenta().bold()),
    );
    let _ = std::io::stdout().flush();
}

/// Draw with ripgrep prompt overlay.
pub fn draw_with_ripgrep(
    renderer: &mut TerminalRenderer,
    state: &AppState,
    filter: Option<&String>,
) {
    draw_main(renderer, state);
    if let Some(f) = filter {
        draw_ripgrep_prompt(renderer, f);
    }
}

fn draw_ripgrep_prompt(renderer: &mut TerminalRenderer, filter: &str) {
    let prompt_row = renderer.rows - FOOTER_ROWS + 1;
    let _ = queue!(
        renderer.writer(),
        cursor::MoveTo(0, prompt_row),
        terminal::Clear(ClearType::UntilNewLine),
        Print(format!("RipGrep: {} [enter/esc]", filter).magenta().bold()),
    );
    let _ = std::io::stdout().flush();
}

/// Draw favorites overlay or main UI.
pub fn draw_with_favorites(
    renderer: &mut TerminalRenderer,
    state: &AppState,
    active: bool,
    items: &[String],
    selected: usize,
) {
    if active {
        draw_favorites_overlay(renderer, items, selected);
    } else {
        draw_main(renderer, state);
    }
}

fn draw_favorites_overlay(renderer: &mut TerminalRenderer, items: &[String], selected: usize) {
    let rows_available = renderer.rows - HEADER_ROWS - FOOTER_ROWS;

    // Header
    let _ = queue!(
        renderer.writer(),
        cursor::MoveTo(0, 0),
        Print("Favorites"),
        terminal::Clear(ClearType::UntilNewLine)
    );

    // Separator line
    let line = (0..renderer.columns).map(|_| "─").collect::<String>();
    let _ = queue!(
        renderer.writer(),
        cursor::MoveTo(0, 1),
        Print(line.with(Color::Blue))
    );

    // Content
    let _ = queue!(renderer.writer(), cursor::MoveTo(0, HEADER_ROWS));

    for (i, item) in items.iter().enumerate() {
        let name = if i == selected {
            item.clone().dark_magenta().negative()
        } else {
            item.clone().dark_magenta()
        };

        let _ = queue!(
            renderer.writer(),
            Print(name),
            terminal::Clear(ClearType::UntilNewLine),
            cursor::MoveToNextLine(1)
        );
    }

    let rows_to_clear: i16 = rows_available as i16 - items.len() as i16;
    if rows_to_clear > 0 {
        for _ in 0..rows_to_clear {
            let _ = queue!(
                renderer.writer(),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
    }

    let _ = std::io::stdout().flush();
}

/// Draw the main UI based on state.
fn draw_main(renderer: &mut TerminalRenderer, state: &AppState) {
    if state.two_pane_mode {
        renderer.draw_two_panes(state);
    } else {
        renderer.draw(state);
    }
}

/// Combined draw function that handles all overlay modes.
pub fn draw(renderer: &mut TerminalRenderer, state: &AppState) {
    draw_main(renderer, state);
}
