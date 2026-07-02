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
    StdFileSystem, SystemClipboard, SystemOpenAdapter, VsCodeAdapter, is_onboarded, load_favorites,
};
use presentation::input_handler::{ContextMenuState, CopyMoveState, Message, TransferUiState};
use presentation::{TerminalRenderer, overlays, run};
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc;
use tui_input::Input;

/// Shell function installed via `eval "$(fishez --init)"` (bash/zsh).
const FZ_INIT: &str = r#"fz() {
  local tmp
  tmp="$(mktemp)"
  command fishez --cwd-file "$tmp" "$@"
  if [ -s "$tmp" ]; then
    cd "$(cat "$tmp")" || true
  fi
  rm -f "$tmp"
}"#;

fn main() {
    setup_panic_handler();

    let args = parse_args(env::args_os().skip(1));
    if args.print_init {
        println!("{FZ_INIT}");
        return;
    }
    let (two_pane, start_path) = (args.two_pane, args.start_path);

    // Initialize infrastructure (adapters)
    let fs = StdFileSystem;
    let mut clipboard = SystemClipboard::new();
    let open = SystemOpenAdapter;
    let vscode = VsCodeAdapter;

    // Initialize application state
    let mut state = AppState::new(two_pane);
    state.show_onboarding = !is_onboarded();
    if let Some(path) = start_path {
        state.left_panel.current_path = path.clone();
        state.right_panel.current_path = path;
    }
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
    let mut transfer_state: Option<TransferUiState> = None;
    let mut move_dest: Option<CopyMoveState> = None;
    let mut context_menu: Option<ContextMenuState> = None;
    let mut favorites_active = false;
    let mut favorites_items = load_favorites();
    let mut favorites_selected: usize = 0;

    let cwd_file = args.cwd_file;

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
        &mut transfer_state,
        &mut move_dest,
        &mut context_menu,
        &mut favorites_active,
        &mut favorites_items,
        &mut favorites_selected,
    );

    if let Some(path) = cwd_file {
        write_cwd_file(&path, &state.active_panel().current_path);
    }
}

struct CliArgs {
    two_pane: bool,
    start_path: Option<PathBuf>,
    cwd_file: Option<PathBuf>,
    print_init: bool,
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> CliArgs {
    let mut parsed = CliArgs {
        two_pane: false,
        start_path: None,
        cwd_file: None,
        print_init: false,
    };

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--two-pane" || arg == "-2" {
            parsed.two_pane = true;
        } else if arg == "--init" {
            parsed.print_init = true;
        } else if arg == "--cwd-file" {
            parsed.cwd_file = args.next().map(PathBuf::from);
        } else if parsed.start_path.is_none() {
            parsed.start_path = Some(PathBuf::from(arg));
        }
    }

    parsed
}

fn write_cwd_file(file: &std::path::Path, dir: &std::path::Path) {
    let _ = std::fs::write(file, dir.display().to_string());
}

fn setup_panic_handler() {
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        let path = std::env::temp_dir().join("fishez-panic.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "panic: {:?}", info);
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::parse_args;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn parses_two_pane_flag_and_start_path() {
        let args = parse_args([OsString::from("--two-pane"), OsString::from("/tmp/project")]);

        assert!(args.two_pane);
        assert_eq!(args.start_path, Some(PathBuf::from("/tmp/project")));
        assert_eq!(args.cwd_file, None);
        assert!(!args.print_init);
    }

    #[test]
    fn parses_cwd_file_and_init() {
        let args = parse_args([
            OsString::from("--cwd-file"),
            OsString::from("/tmp/out"),
            OsString::from("--init"),
        ]);

        assert_eq!(args.cwd_file, Some(PathBuf::from("/tmp/out")));
        assert!(args.print_init);
        assert_eq!(args.start_path, None);
    }

    #[test]
    fn write_cwd_file_writes_panel_dir() {
        let file = std::env::temp_dir().join("fishez-cwd-test");
        super::write_cwd_file(&file, &PathBuf::from("/some/dir"));

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "/some/dir");
        let _ = std::fs::remove_file(&file);
    }
}
