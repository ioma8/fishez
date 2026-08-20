//! Shell installer - wires the `fz` shell function into the user's rc files.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Shell function installed via `eval "$(fishez --init)"` (bash/zsh).
pub const FZ_INIT: &str = r#"fz() {
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

pub fn install_shell() -> io::Result<String> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    let shell = env::var("SHELL").unwrap_or_default();

    install_shell_for(&home, &shell)
}

pub fn uninstall_shell() -> io::Result<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_shell_line_appends_once() {
        let file = std::env::temp_dir().join("fishez-shell-test");
        let _ = std::fs::remove_file(&file);

        install_shell_line(&file, "eval \"$(fishez --init)\"").unwrap();
        install_shell_line(&file, "eval \"$(fishez --init)\"").unwrap();

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

        uninstall_shell_line(&file).unwrap();

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

        install_shell_line(&file, "eval \"$(fishez --init)\"").unwrap();

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

        let err = install_fish_function(&file).unwrap_err();

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

        let err = uninstall_fish_function(&file).unwrap_err();

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

        install_shell_for(&home, "/bin/bash").unwrap();

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

        uninstall_shell_for(&home, "/bin/bash").unwrap();

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
}
