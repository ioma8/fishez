//! Terminal overlays - modal prompts and dialogs.

use crate::application::AppState;
use crate::presentation::input_handler::ContextMenuState;
use crate::presentation::{FOOTER_ROWS, HEADER_ROWS, TerminalRenderer};
use crossterm::style::{Color, Print, Stylize};
use crossterm::terminal::ClearType;
use crossterm::{cursor, queue, terminal};
use std::io::Write;
use std::path::PathBuf;
use tui_input::Input;

/// Draw the main UI based on state.
pub fn draw(renderer: &mut TerminalRenderer, state: &AppState) {
    if state.two_pane_mode {
        renderer.draw_two_panes(state);
    } else {
        renderer.draw(state);
    }
}

/// Draw with delete confirmation overlay.
pub fn draw_with_delete(
    renderer: &mut TerminalRenderer,
    state: &AppState,
    paths: Option<&Vec<PathBuf>>,
) {
    draw(renderer, state);
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
pub fn draw_with_find(renderer: &mut TerminalRenderer, state: &AppState, filter: Option<&Input>) {
    draw(renderer, state);
    if let Some(f) = filter {
        draw_input_prompt(renderer, "Find", f);
    }
}

/// Draw with ripgrep prompt overlay.
pub fn draw_with_ripgrep(
    renderer: &mut TerminalRenderer,
    state: &AppState,
    filter: Option<&Input>,
) {
    draw(renderer, state);
    if let Some(f) = filter {
        draw_input_prompt(renderer, "RipGrep", f);
    }
}

/// Draw with shell command prompt overlay.
pub fn draw_with_shell(
    renderer: &mut TerminalRenderer,
    state: &AppState,
    command: Option<&Input>,
) {
    draw(renderer, state);
    if let Some(c) = command {
        draw_input_prompt(renderer, "!", c);
    }
}

/// Draw with rename prompt overlay.
pub fn draw_with_rename(renderer: &mut TerminalRenderer, state: &AppState, input: Option<&Input>) {
    draw(renderer, state);
    if let Some(inp) = input {
        draw_input_prompt(renderer, "Rename", inp);
    }
}

/// Draw with new folder prompt overlay.
pub fn draw_with_new_folder(
    renderer: &mut TerminalRenderer,
    state: &AppState,
    input: Option<&Input>,
) {
    draw(renderer, state);
    if let Some(inp) = input {
        draw_input_prompt(renderer, "New folder", inp);
    }
}

/// Draw with copy destination prompt overlay.
pub fn draw_with_copy_dest(renderer: &mut TerminalRenderer, state: &AppState, dest: &Input) {
    draw(renderer, state);
    draw_input_prompt(renderer, "Copy to", dest);
}

/// Draw with move destination prompt overlay.
pub fn draw_with_move_dest(renderer: &mut TerminalRenderer, state: &AppState, dest: &Input) {
    draw(renderer, state);
    draw_input_prompt(renderer, "Move to", dest);
}

fn draw_input_prompt(renderer: &mut TerminalRenderer, label: &str, input: &Input) {
    let prompt_row = renderer.rows - FOOTER_ROWS + 1;
    let value = input.value();
    let cursor = input.cursor();

    let before = &value[..cursor];
    let after = &value[cursor..];
    let mut after_chars = after.chars();
    let cur_ch = after_chars.next();
    let rest = after_chars.as_str();

    let _ = queue!(
        renderer.writer(),
        cursor::MoveTo(0, prompt_row),
        terminal::Clear(ClearType::UntilNewLine),
        Print(format!("{}: ", label).cyan().bold()),
        Print(before.cyan().bold()),
    );
    match cur_ch {
        Some(ch) => {
            let _ = queue!(renderer.writer(),
                Print(ch.to_string().black().on_cyan()),
                Print(rest.cyan().bold()),
            );
        }
        None => {
            let _ = queue!(renderer.writer(), Print(" ".black().on_cyan()));
        }
    }
    let _ = queue!(renderer.writer(), Print("  [esc]".dark_grey()));
    let _ = std::io::stdout().flush();
}

/// Draw a floating context menu on top of the current view.
pub fn draw_context_menu(renderer: &mut TerminalRenderer, ctx: &ContextMenuState) {
    let labels: Vec<&str> = ctx.actions.iter().map(|(l, _)| *l).collect();
    let width = labels.iter().map(|l| l.len()).max().unwrap_or(8) as u16 + 4;
    let height = labels.len() as u16 + 2;

    // Clamp so the menu stays on screen.
    let col = ctx.col.min(renderer.columns.saturating_sub(width));
    let row = ctx.row.min(renderer.rows.saturating_sub(height));

    let top = format!("┌{}┐", "─".repeat(width as usize - 2));
    let bot = format!("└{}┘", "─".repeat(width as usize - 2));

    let _ = queue!(renderer.writer(), cursor::MoveTo(col, row), Print(top.clone().white()));

    for (i, label) in labels.iter().enumerate() {
        let content = format!(" {:<width$} ", label, width = width as usize - 4);
        let styled = if i == ctx.selected {
            format!("│{}│", content).black().on_white()
        } else {
            format!("│{}│", content).white()
        };
        let _ = queue!(
            renderer.writer(),
            cursor::MoveTo(col, row + 1 + i as u16),
            Print(styled)
        );
    }

    let _ = queue!(renderer.writer(), cursor::MoveTo(col, row + height - 1), Print(bot.white()));
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
        draw(renderer, state);
    }
}

fn draw_favorites_overlay(renderer: &mut TerminalRenderer, items: &[String], selected: usize) {
    let rows_available = renderer.rows - HEADER_ROWS - FOOTER_ROWS;

    let _ = queue!(
        renderer.writer(),
        cursor::MoveTo(0, 0),
        Print("Favorites"),
        terminal::Clear(ClearType::UntilNewLine)
    );

    let line = "─".repeat(renderer.columns as usize);
    let _ = queue!(
        renderer.writer(),
        cursor::MoveTo(0, 1),
        Print(line.with(Color::Blue))
    );

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

    let rows_to_clear = (rows_available as i16 - items.len() as i16).max(0) as u16;
    for _ in 0..rows_to_clear {
        let _ = queue!(
            renderer.writer(),
            terminal::Clear(ClearType::UntilNewLine),
            cursor::MoveToNextLine(1)
        );
    }

    let _ = std::io::stdout().flush();
}
