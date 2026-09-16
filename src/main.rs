//! Fishez - A terminal file manager using Clean Architecture.
//!
//! Composition Root: This is where all the layers are wired together.

mod application;
mod domain;
mod infrastructure;
mod presentation;
mod shell_install;
#[cfg(test)]
mod test_support;

use application::AppState;
use application::use_cases::navigate;
use infrastructure::{
    StdFileSystem, SystemClipboard, SystemOpenAdapter, VsCodeAdapter, is_onboarded, load_favorites,
    load_recents, save_recents,
};
use presentation::input_handler::{Message, UiState};
use presentation::{TerminalRenderer, overlays, run};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

fn main() {
    setup_panic_handler();

    let args = parse_args(env::args_os().skip(1));
    if args.print_init {
        println!("{}", shell_install::FZ_INIT);
        return;
    }
    if args.install_shell {
        match shell_install::install_shell() {
            Ok(message) => println!("{message}"),
            Err(err) => {
                eprintln!("fishez: {err}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.uninstall_shell {
        match shell_install::uninstall_shell() {
            Ok(message) => println!("{message}"),
            Err(err) => {
                eprintln!("fishez: {err}");
                std::process::exit(1);
            }
        }
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

    // Modal-overlay and feature state
    let mut ui = UiState {
        favorites_items: load_favorites(),
        recent_items: load_recents(),
        ..Default::default()
    };
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
        &mut ui,
    );
    save_recents(&ui.recent_items);

    if let Some(path) = cwd_file {
        write_cwd_file(&path, &state.active_panel().current_path);
    }
}

struct CliArgs {
    two_pane: bool,
    start_path: Option<PathBuf>,
    cwd_file: Option<PathBuf>,
    print_init: bool,
    install_shell: bool,
    uninstall_shell: bool,
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> CliArgs {
    let mut parsed = CliArgs {
        two_pane: false,
        start_path: None,
        cwd_file: None,
        print_init: false,
        install_shell: false,
        uninstall_shell: false,
    };

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--two-pane" || arg == "-2" {
            parsed.two_pane = true;
        } else if arg == "--init" {
            parsed.print_init = true;
        } else if arg == "--install-shell" {
            parsed.install_shell = true;
        } else if arg == "--uninstall-shell" {
            parsed.uninstall_shell = true;
        } else if arg == "--cwd-file" {
            parsed.cwd_file = args.next().map(PathBuf::from);
        } else if parsed.start_path.is_none() {
            parsed.start_path = Some(PathBuf::from(arg));
        }
    }

    parsed
}

fn write_cwd_file(file: &Path, dir: &Path) {
    let _ = fs::write(file, dir.display().to_string());
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
        assert!(!args.install_shell);
        assert!(!args.uninstall_shell);
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
        assert!(!args.install_shell);
        assert!(!args.uninstall_shell);
        assert_eq!(args.start_path, None);
    }

    #[test]
    fn parses_install_shell() {
        let args = parse_args([OsString::from("--install-shell")]);

        assert!(args.install_shell);
        assert!(!args.uninstall_shell);
        assert!(!args.print_init);
        assert_eq!(args.start_path, None);
    }

    #[test]
    fn parses_uninstall_shell() {
        let args = parse_args([OsString::from("--uninstall-shell")]);

        assert!(args.uninstall_shell);
        assert!(!args.install_shell);
        assert!(!args.print_init);
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
