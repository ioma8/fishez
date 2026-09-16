<div align="center">

<img src="Fishez_logo.svg" alt="fishez logo" width="180" />

# fishez

**Total Commander muscle memory, in your terminal.**

[![CI](https://github.com/ioma8/fishez/actions/workflows/ci.yml/badge.svg)](https://github.com/ioma8/fishez/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org/)

Type to filter. F3 to preview. F5 to copy. Leave your shell in the directory you found.

A keyboard-driven file manager for macOS and Linux, with dual panes, familiar function keys, and built-in actions for fd, ripgrep, and your editor.

<img src="demo.gif" alt="fishez demo" width="900" />

**Start with two keys:** `F3` / `Ctrl+P` quick-views the selected file. `F4` / `Ctrl+O` opens it in your editor.

</div>

## 🚀 Install

### Homebrew, recommended

```bash
brew install ioma8/tap/fishez
fishez --install-shell
```

Open a new terminal (or source the file printed by the installer), then run:

```bash
fz
```

Homebrew includes `fd` and `ripgrep`. `bat` is optional for text previews.

### Your first minute

1. Run `fz` inside a project and start typing part of a filename to filter the current directory.
2. Press `Esc` to clear the filter. Use the arrow keys and `Enter` to navigate.
3. Press `F3` / `Ctrl+P` to preview a file; press `Esc` to close the preview.
4. Press `Ctrl+T` for two panes and `Tab` to switch between them. `F5` / `Ctrl+Y` copies to the other pane.
5. From the normal browsing view, press `Esc` to quit. Run `pwd`: your shell is now in the directory you were browsing.

On laptops where function keys control brightness or volume, use the Ctrl alternatives above. `F1` opens help.

See [a Total Commander-style workflow on macOS and Linux](docs/commander-workflow.md) for a complete example.

### One-line binary installer

```bash
curl -sfL https://raw.githubusercontent.com/ioma8/fishez/main/install.sh | sh
```

The installer downloads the latest GitHub release and runs `fishez --install-shell` when it can.

### Build with Cargo

Requires Rust and CMake.

```bash
cargo install fishez
fishez --install-shell
fz
```

### Enable `fz`

`fz` is a tiny shell function around `fishez`. It opens the file manager and, when you quit, changes your shell into the directory you were browsing. Use `fishez` directly if you want to browse without changing the parent shell directory.

Run this once after installing with Homebrew or Cargo:

```bash
fishez --install-shell
```

It detects zsh, bash, or fish and installs the `fz` wrapper if it is not already present. Then open a new terminal, or source the file it prints.

To remove the wrapper later:

```bash
fishez --uninstall-shell
```

If you prefer to do it manually, append the setup line to your shell rc file:

```bash
# zsh
echo 'eval "$(fishez --init)"' >> ~/.zshrc
source ~/.zshrc

# bash
echo 'eval "$(fishez --init)"' >> ~/.bashrc
source ~/.bashrc

# bash login shells, common on older macOS setups
echo 'eval "$(fishez --init)"' >> ~/.bash_profile
source ~/.bash_profile
```

Fish uses a native function instead:

```fish
function fz
    set tmp (mktemp)
    command fishez --cwd-file $tmp $argv
    if test -s $tmp
        cd (cat $tmp)
    end
    rm -f $tmp
end
funcsave fz
```

Now run:

```bash
fz ~/projects
```

## Why fishez?

- **Fast browsing beats `cd` guessing.** Open `fz`, move visually, quit in the right directory.
- **Inline filtering is the superpower.** Start typing and the current directory narrows instantly.
- **Quick view keeps you in flow.** Preview code, text, directories, images (including animated GIFs), and RAW thumbnails before opening.
- **Commander-style controls feel familiar.** Function keys, selection, dual panes, copy/move/delete, no plugin setup.

fishez is a keyboard-driven terminal file manager for developers who want fast file browsing without leaving the terminal.

Press `Ctrl+T` for two panes, `F3` / `Ctrl+P` for quick view, `F4` / `Ctrl+O` for editor, and `F1` for help.

## 🎯 Core Features

| Feature | Shortcut | Description |
|---------|----------|-------------|
| **Navigate** | `↑↓` / `Enter` / `Backspace` | Move through directories instantly |
| **Quick View** | `F3` / `Ctrl+P` | Preview files with built-in highlighting or optional `bat` |
| **Multi-select** | `Space` | Batch operations on multiple files |
| **Copy** | `F5` / `Ctrl+Y` | Copy selected file(s) to opposite pane or prompted destination, with background progress and overwrite prompts |
| **Move** | `F6` | Move selected file(s) to opposite pane or prompted destination, with background progress and overwrite prompts |
| **Rename** | `Shift+F6` | Inline rename of file or directory under cursor |
| **New Folder** | `F7` | Create a new directory in current pane |
| **Delete** | `F8` / `Ctrl+W` | Move to trash (safe delete) |
| **Open in editor** | `F4` / `Ctrl+O` | Uses `$EDITOR`, then falls back to VS Code |
| **Find Files** | `Ctrl+F` | Find files with `fd` |
| **Content Search** | `Ctrl+R` | Search file contents with `ripgrep` |
| **Shell Command** | `!` | Run a shell command; use `{1}` for selected file, `{@}` for all selected |
| **Favorites** | `Ctrl+D` | Quick-jump to pinned directories (stored in `~/.fishez/favorites.txt`) |
| **Recent directories** | `Ctrl+J` | Jump to recently visited directories (stored in `~/.fishez/recent.txt`) |
| **Hidden files** | `Ctrl+H` | Toggle dotfiles |
| **Filter** | *Start typing* | Instantly filter current directory |
| **Help** | `F1` | Show all shortcuts |

### Dual-Pane Mode

Press `Ctrl+T` for a split view. `-2` / `--two-pane` still work as startup aliases, but they are the same toggle.

### Safe Copy & Move

Copy (`F5`) and move (`F6`) run in the background, so the UI stays responsive during large transfers — a progress line shows the current file and how many are done. If a destination file already exists, fishez pauses and asks: **(O)verwrite**, **(S)kip**, **Overwrite (A)ll**, **Skip a(L)l**, or **(Esc)** to cancel the whole transfer. Copying or moving a directory onto an existing one merges the two instead of silently replacing it. Press `Esc` at any time to stop a transfer after the current file.

## Is it for you?

Try fishez if you like Commander-style function keys, typing directly to filter, and switching between browsing and your shell. These defaults are the focus; you don't need to configure a keymap to try the workflow.

Other terminal file managers also offer previews, search, and shell integration. The reason to choose fishez is whether its interaction style fits your habits. It is an independent project, not affiliated with Total Commander.

### Platform and preview support

Release binaries and CI builds cover macOS and Linux on ARM64 and x86-64. Windows is not currently covered by the release/CI matrix.

Text previews work without an image-capable terminal. Inline images and animated GIFs require kitty-graphics or iTerm2 image support. RAW previews use embedded thumbnails from formats supported by `jpgfromraw-lib`.

Content-search results retain their first matching line; quick view and editor actions open at that line.

## Feedback from your first real task

Try finding, previewing, and copying a file in a project you already use. If something gets in the way, [open an issue](https://github.com/ioma8/fishez/issues/new) with your OS, terminal, installation method, and what you expected to happen. Feedback about why you returned to another tool is welcome too.

If fishez earns a place in your workflow, a GitHub star or a demo shared with a friend helps others discover it.

## 📦 Requirements

fishez works standalone but shines with these tools installed:

- **[fd](https://github.com/sharkdp/fd)** — Fast file search (`Ctrl+F`)
- **[ripgrep](https://github.com/BurntSushi/ripgrep)** — Content search (`Ctrl+R`)
- **[bat](https://github.com/sharkdp/bat)** — Optional colorized text previews
- **`$EDITOR` or VS Code CLI** — Editor integration (`F4` / `Ctrl+O`)

```bash
# macOS
brew install fd ripgrep bat

# Ubuntu/Debian
sudo apt install fd-find ripgrep bat

# Arch
sudo pacman -S fd ripgrep bat
```

## ⌨️ All Shortcuts

```
Navigation                    File Operations
─────────────────────────     ─────────────────────────
↑/↓         Move cursor       F5/Ctrl+Y   Copy selected
Home/End    Jump to start/end F6          Move selected
Tab         Switch pane       Shift+F6    Rename
                              F7          New folder
Quick View (F3 / Ctrl+P)      F8/Ctrl+W   Delete selected
─────────────────────────
↑/↓         Scroll content    Actions
←/→         Prev/next file    ─────────────────────────
PgUp/PgDn   Page scroll       Enter       Open / Enter dir
Esc         Close preview     Backspace   Go up one level
                              Space       Toggle selection
Favorites (Ctrl+D)            Ctrl+Enter  Copy path
─────────────────────────     Ctrl+Shift+Enter  Copy absolute path
↑/↓         Navigate
Enter       Jump to favorite  Search & Tools
Ctrl+Shift+D  Add current dir ─────────────────────────
                              F4/Ctrl+O   Open in editor
                              Ctrl+F      fd file search
                              Ctrl+R      ripgrep search
                              Ctrl+H      Toggle hidden files
                              !           Shell command
                              F1          Show help
                              Esc/F10/Ctrl+C  Quit
```

## 🤝 Contributing

Contributions are welcome! The codebase follows Clean Architecture:

```
src/
├── domain/          # Pure business entities
├── application/     # Use cases & ports
├── infrastructure/  # External adapters (fs, clipboard, search)
├── presentation/    # Terminal UI & input handling
└── main.rs          # Composition root
```

```bash
cargo check     # Type check
cargo fmt       # Format code
cargo clippy    # Lint
cargo test      # Run tests
```

Additional docs live under [`docs/`](docs).

### Regenerating the demo GIF

The README demo is a scripted [vhs](https://github.com/charmbracelet/vhs) recording, so it can be re-recorded any time the UI changes:

```bash
brew install vhs ttyd bat edit   # recording tools + preview/editor used in the tape
cargo build --release
eval "$(TMPDIR=/tmp bash scripts/demo_setup.sh)"   # stages the demo project, exports FISHEZ_DEMO_DIR
FISHEZ_BIN=/tmp/fishez_target/release/fishez vhs demo.tape   # target dir is set in .cargo/config.toml
```

This rewrites `demo.gif`. The tape sets `EDITOR=edit` for the editor scene and uses the `Ctrl+P`/`Ctrl+O` aliases because vhs cannot send F-keys.

### CI Integration Tests

CI runs extra integration tests that are gated locally to avoid opening files
or moving items to trash during development. To run them locally:

```bash
FISHEZ_RUN_OPEN_TESTS=1 FISHEZ_RUN_TRASH_TESTS=1 cargo test --test integration_flows
```

On Linux CI, the workflow installs `fd`, `ripgrep`, and `xdg-utils` to support
these tests and the file open adapter.

## 📄 License

MIT © [ioma8](https://github.com/ioma8)
