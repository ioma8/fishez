//! Fishez - A terminal file manager using Clean Architecture.
//!
//! Composition Root: This is where all the layers are wired together.

mod application;
mod domain;
mod infrastructure;
mod logger;
mod presentation;

use application::AppState;
use application::use_cases::navigate;
use infrastructure::{
    StdFileSystem, SystemClipboard, SystemOpenAdapter, VsCodeAdapter, load_favorites,
};
use presentation::input_handler::Message;
use presentation::{TerminalRenderer, overlays, run};
use std::env;
use std::path::PathBuf;
use std::sync::mpsc;

fn main() {
    setup_panic_handler();

    let two_pane = env::args().any(|arg| arg == "--two-pane" || arg == "-2");

    // Initialize infrastructure (adapters)
    let fs = StdFileSystem::new();
    let mut clipboard = SystemClipboard::new();
    let open = SystemOpenAdapter::new();
    let vscode = VsCodeAdapter::new();

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
    let mut find_filter: Option<String> = None;
    let mut ripgrep_filter: Option<String> = None;
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
        &mut favorites_active,
        &mut favorites_items,
        &mut favorites_selected,
        two_pane,
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
