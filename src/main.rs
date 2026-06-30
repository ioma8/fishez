//! Fishez - A terminal file manager using Clean Architecture.
//!
//! Composition Root: This is where all the layers are wired together.

mod application;
mod domain;
mod infrastructure;
mod presentation;
#[cfg(test)]
mod test_support;

use application::AppState;
use application::use_cases::navigate;
use infrastructure::{
    StdFileSystem, SystemClipboard, SystemOpenAdapter, VsCodeAdapter, load_favorites,
};
use presentation::input_handler::{CopyMoveState, ContextMenuState, Message};
use tui_input::Input;
use presentation::{TerminalRenderer, overlays, run};
use std::env;
use std::path::PathBuf;
use std::sync::mpsc;

fn main() {
    setup_panic_handler();

    let two_pane = env::args().any(|arg| arg == "--two-pane" || arg == "-2");

    // Initialize infrastructure (adapters)
    let fs = StdFileSystem;
    let mut clipboard = SystemClipboard::new();
    let open = SystemOpenAdapter;
    let vscode = VsCodeAdapter;

    // Initialize application state
    let mut state = AppState::new(two_pane);
    navigate::refresh_entries(&fs, &mut state.left_panel);
    navigate::refresh_entries(&fs, &mut state.right_panel);

    // Initialize presentation (renderer)
    let mut renderer = TerminalRenderer::new();

    // Async communication channel
    let (sender, receiver) = mpsc::channel::<Message>();

    // Feature state
    let mut delete_paths: Option<Vec<PathBuf>> = None;
    let mut find_filter: Option<Input> = None;
    let mut ripgrep_filter: Option<Input> = None;
    let mut shell_command: Option<Input> = None;
    let mut shell_history: Vec<String> = Vec::new();
    let mut shell_history_idx: Option<usize> = None;
    let mut rename_input: Option<Input> = None;
    let mut new_folder_input: Option<Input> = None;
    let mut copy_dest: Option<CopyMoveState> = None;
    let mut move_dest: Option<CopyMoveState> = None;
    let mut context_menu: Option<ContextMenuState> = None;
    let mut favorites_active = false;
    let mut favorites_items = load_favorites();
    let mut favorites_selected: usize = 0;

    // Initial draw and run event loop
    overlays::draw(&mut renderer, &state);
    run(
        &mut state,
        &mut renderer,
        &fs,
        &open,
        &vscode,
        &mut clipboard,
        &sender,
        &receiver,
        &mut delete_paths,
        &mut find_filter,
        &mut ripgrep_filter,
        &mut shell_command,
        &mut shell_history,
        &mut shell_history_idx,
        &mut rename_input,
        &mut new_folder_input,
        &mut copy_dest,
        &mut move_dest,
        &mut context_menu,
        &mut favorites_active,
        &mut favorites_items,
        &mut favorites_selected,
    );
}

fn setup_panic_handler() {
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("err.txt")
        {
            let _ = writeln!(f, "panic: {:?}", info);
        }
    }));
}
