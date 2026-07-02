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
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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
const FZ_INIT_LINE: &str = r#"eval "$(fishez --init)""#;
const FISH_FZ_INIT: &str = r#"function fz
    set tmp (mktemp)
    command fishez --cwd-file $tmp $argv
    if test -s $tmp
        cd (cat $tmp)
    end
    rm -f $tmp
end
"#;

fn main() {
    setup_panic_handler();

    let args = parse_args(env::args_os().skip(1));
    if args.print_init {
        println!("{FZ_INIT}");
        return;
    }
    if args.install_shell {
        match install_shell() {
            Ok(message) => println!("{message}"),
            Err(err) => {
                eprintln!("fishez: {err}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.uninstall_shell {
        match uninstall_shell() {
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

fn install_shell() -> io::Result<String> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    let shell = env::var("SHELL").unwrap_or_default();

    install_shell_for(&home, &shell)
}

fn uninstall_shell() -> io::Result<String> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    let shell = env::var("SHELL").unwrap_or_default();

    uninstall_shell_for(&home, &shell)
}

fn install_shell_for(home: &Path, shell: &str) -> io::Result<String> {
    let shell_name = Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    match shell_name {
        "zsh" => install_posix_shell_line(&home.join(".zshrc"), "source ~/.zshrc"),
        "bash" => {
            let rc_file = bash_rc_file(home);
            let reload = if rc_file.ends_with(".bash_profile") {
                "source ~/.bash_profile"
            } else {
                "source ~/.bashrc"
            };
            install_posix_shell_line(&rc_file, reload)
        }
        "fish" => install_fish_function(&home.join(".config/fish/functions/fz.fish")),
        _ => Err(io::Error::other(format!(
            "unsupported shell '{shell_name}'. Add this manually to your shell rc: {FZ_INIT_LINE}"
        ))),
    }
}

fn uninstall_shell_for(home: &Path, shell: &str) -> io::Result<String> {
    let shell_name = Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    match shell_name {
        "zsh" => uninstall_posix_shell_line(&home.join(".zshrc"), "source ~/.zshrc"),
        "bash" => uninstall_bash_shell_lines(home),
        "fish" => uninstall_fish_function(&home.join(".config/fish/functions/fz.fish")),
        _ => Err(io::Error::other(format!(
            "unsupported shell '{shell_name}'. Remove any fishez --init line from your shell rc"
        ))),
    }
}

fn bash_rc_file(home: &Path) -> PathBuf {
    let bash_profile = home.join(".bash_profile");
    if bash_profile.exists() {
        bash_profile
    } else {
        home.join(".bashrc")
    }
}

fn install_posix_shell_line(rc_file: &Path, reload: &str) -> io::Result<String> {
    let changed = install_shell_line(rc_file, FZ_INIT_LINE)?;
    let status = if changed {
        "Added"
    } else {
        "Already present in"
    };

    Ok(format!(
        "{status} fz setup: {}\nOpen a new shell or run: {reload}",
        rc_file.display()
    ))
}

fn uninstall_posix_shell_line(rc_file: &Path, reload: &str) -> io::Result<String> {
    let changed = uninstall_shell_line(rc_file)?;
    let status = if changed { "Removed" } else { "Not present in" };

    Ok(format!(
        "{status} fz setup: {}\nOpen a new shell or run: {reload}",
        rc_file.display()
    ))
}

fn uninstall_bash_shell_lines(home: &Path) -> io::Result<String> {
    let bash_profile = home.join(".bash_profile");
    let bashrc = home.join(".bashrc");
    let removed_profile = uninstall_shell_line(&bash_profile)?;
    let removed_bashrc = uninstall_shell_line(&bashrc)?;
    let status = if removed_profile || removed_bashrc {
        "Removed"
    } else {
        "Not present in"
    };

    Ok(format!(
        "{status} fz setup: {} and {}\nOpen a new shell",
        bash_profile.display(),
        bashrc.display()
    ))
}

fn install_shell_line(rc_file: &Path, line: &str) -> io::Result<bool> {
    let content = fs::read_to_string(rc_file).unwrap_or_default();
    if content.lines().any(is_active_fishez_init_line) {
        return Ok(false);
    }

    if let Some(parent) = rc_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rc_file)?;
    if !content.is_empty() && !content.ends_with('\n') {
        writeln!(file)?;
    }
    writeln!(file, "{line}")?;

    Ok(true)
}

fn uninstall_shell_line(rc_file: &Path) -> io::Result<bool> {
    let content = match fs::read_to_string(rc_file) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };
    let mut removed = false;
    let kept = content
        .lines()
        .filter(|line| {
            let remove = is_active_fishez_init_line(line);
            removed |= remove;
            !remove
        })
        .collect::<Vec<_>>()
        .join("\n");

    if removed {
        let trailing_newline = if kept.is_empty() { "" } else { "\n" };
        fs::write(rc_file, format!("{kept}{trailing_newline}"))?;
    }

    Ok(removed)
}

fn is_active_fishez_init_line(line: &str) -> bool {
    line.trim_start().starts_with(FZ_INIT_LINE)
}

fn install_fish_function(function_file: &Path) -> io::Result<String> {
    if let Some(parent) = function_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let changed = match fs::read_to_string(function_file) {
        Ok(content) if content == FISH_FZ_INIT => false,
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "refusing to overwrite existing fish function: {}",
                    function_file.display()
                ),
            ));
        }
        _ => {
            fs::write(function_file, FISH_FZ_INIT)?;
            true
        }
    };
    let status = if changed {
        "Added"
    } else {
        "Already present in"
    };

    Ok(format!(
        "{status} fz setup: {}\nOpen a new shell or run: source {}",
        function_file.display(),
        function_file.display()
    ))
}

fn uninstall_fish_function(function_file: &Path) -> io::Result<String> {
    let changed = match fs::read_to_string(function_file) {
        Ok(content) if content == FISH_FZ_INIT => {
            fs::remove_file(function_file)?;
            true
        }
        Ok(content) if content.contains("command fishez --cwd-file") => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "refusing to delete customized fish function: {}",
                    function_file.display()
                ),
            ));
        }
        Ok(_) => false,
        Err(err) if err.kind() == io::ErrorKind::NotFound => false,
        Err(err) => return Err(err),
    };
    let status = if changed { "Removed" } else { "Not present at" };

    Ok(format!(
        "{status} fz setup: {}\nOpen a new shell to unload any existing fz function",
        function_file.display()
    ))
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
    fn install_shell_line_appends_once() {
        let file = std::env::temp_dir().join("fishez-shell-test");
        let _ = std::fs::remove_file(&file);

        super::install_shell_line(&file, "eval \"$(fishez --init)\"").unwrap();
        super::install_shell_line(&file, "eval \"$(fishez --init)\"").unwrap();

        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content.matches("fishez --init").count(), 1);
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn uninstall_shell_line_removes_only_fishez_init() {
        let file = std::env::temp_dir().join("fishez-shell-uninstall-test");
        std::fs::write(
            &file,
            "export PATH=/tmp:$PATH\neval \"$(fishez --init)\"\nalias ll='ls -la'\n",
        )
        .unwrap();

        super::uninstall_shell_line(&file).unwrap();

        let content = std::fs::read_to_string(&file).unwrap();
        assert!(!content.contains("fishez --init"));
        assert!(content.contains("export PATH=/tmp:$PATH"));
        assert!(content.contains("alias ll='ls -la'"));
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn install_shell_line_ignores_commented_fishez_init() {
        let file = std::env::temp_dir().join("fishez-commented-install-test");
        std::fs::write(&file, "# eval \"$(fishez --init)\"\n").unwrap();

        super::install_shell_line(&file, "eval \"$(fishez --init)\"").unwrap();

        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content.matches("fishez --init").count(), 2);
        assert!(content.contains("# eval \"$(fishez --init)\""));
        assert!(content.contains("\neval \"$(fishez --init)\""));
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn uninstall_shell_line_preserves_commented_fishez_init() {
        let file = std::env::temp_dir().join("fishez-commented-uninstall-test");
        std::fs::write(
            &file,
            "# eval \"$(fishez --init)\"\neval \"$(fishez --init)\" # old installer\n",
        )
        .unwrap();

        super::uninstall_shell_line(&file).unwrap();

        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "# eval \"$(fishez --init)\"\n");
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn install_fish_function_refuses_to_overwrite_custom_fz() {
        let file = std::env::temp_dir().join("fishez-custom-fz.fish");
        std::fs::write(&file, "function fz\n    echo custom\nend\n").unwrap();

        let err = super::install_fish_function(&file).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "function fz\n    echo custom\nend\n"
        );
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn uninstall_fish_function_refuses_to_delete_custom_fishez_fz() {
        let file = std::env::temp_dir().join("fishez-custom-fishez-fz.fish");
        let custom =
            "function fz\n    command fishez --cwd-file /tmp/fz $argv\n    echo custom\nend\n";
        std::fs::write(&file, custom).unwrap();

        let err = super::uninstall_fish_function(&file).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), custom);
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn bash_install_prefers_existing_bash_profile() {
        let home = std::env::temp_dir().join("fishez-bash-home");
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".bash_profile"), "# login shell\n").unwrap();

        super::install_shell_for(&home, "/bin/bash").unwrap();

        assert!(
            std::fs::read_to_string(home.join(".bash_profile"))
                .unwrap()
                .contains("fishez --init")
        );
        assert!(!home.join(".bashrc").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn bash_uninstall_removes_profile_and_bashrc_lines() {
        let home = std::env::temp_dir().join("fishez-bash-uninstall-home");
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".bash_profile"), "eval \"$(fishez --init)\"\n").unwrap();
        std::fs::write(home.join(".bashrc"), "eval \"$(fishez --init)\"\n").unwrap();

        super::uninstall_shell_for(&home, "/bin/bash").unwrap();

        assert!(
            !std::fs::read_to_string(home.join(".bash_profile"))
                .unwrap()
                .contains("fishez --init")
        );
        assert!(
            !std::fs::read_to_string(home.join(".bashrc"))
                .unwrap()
                .contains("fishez --init")
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn write_cwd_file_writes_panel_dir() {
        let file = std::env::temp_dir().join("fishez-cwd-test");
        super::write_cwd_file(&file, &PathBuf::from("/some/dir"));

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "/some/dir");
        let _ = std::fs::remove_file(&file);
    }
}
