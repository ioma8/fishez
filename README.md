<div align="center">

<img src="Fishez_logo.svg" alt="fishez logo" width="180" />

# fishez

**A lightning-fast terminal file manager built for developers**

[![CI](https://github.com/ioma8/fishez/actions/workflows/ci.yml/badge.svg)](https://github.com/ioma8/fishez/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org/)

Navigate files at the speed of thought. Preview code with syntax highlighting. Search instantly with ripgrep. Open in VS Code with one keystroke.

</div>

## 🚀 Quick Start

```bash
cargo install fishez
fishez ~/projects
```

Press `Ctrl+T` to split into two panes. Press `Esc` to back out of overlays, and from a clean normal state it quits.

## ✨ What is fishez?

fishez is a keyboard-driven terminal file manager for developers who live in the terminal. No config file, no plugin hunt, no startup ceremony.

## 🎯 Core Features

| Feature | Shortcut | Description |
|---------|----------|-------------|
| **Navigate** | `↑↓` / `Enter` / `Backspace` | Move through directories instantly |
| **Quick View** | `F3` | Preview files with syntax highlighting |
| **Multi-select** | `Space` | Batch operations on multiple files |
| **Copy** | `F5` | Copy selected file(s) to opposite pane or prompted destination, with background progress and overwrite prompts |
| **Move** | `F6` | Move selected file(s) to opposite pane or prompted destination, with background progress and overwrite prompts |
| **Rename** | `Shift+F6` | Inline rename of file or directory under cursor |
| **New Folder** | `F7` | Create a new directory in current pane |
| **Delete** | `F8` / `Ctrl+W` | Move to trash (safe delete) |
| **Open in VS Code** | `F4` | Jump straight into your editor |
| **Find Files** | `Ctrl+F` | Find files with `fd` |
| **Content Search** | `Ctrl+R` | Search file contents with `ripgrep` |
| **Shell Command** | `!` | Run a shell command; use `{1}` for selected file, `{@}` for all selected |
| **Favorites** | `Ctrl+D` | Quick-jump to pinned directories |
| **Filter** | *Start typing* | Instantly filter current directory |
| **Help** | `F1` | Show all shortcuts |

### Dual-Pane Mode

Press `Ctrl+T` for a split view. `-2` / `--two-pane` still work as startup aliases, but they are the same toggle.

### Safe Copy & Move

Copy (`F5`) and move (`F6`) run in the background, so the UI stays responsive during large transfers — a progress line shows the current file and how many are done. If a destination file already exists, fishez pauses and asks: **(O)verwrite**, **(S)kip**, **Overwrite (A)ll**, **Skip a(L)l**, or **(Esc)** to cancel the whole transfer. Copying or moving a directory onto an existing one merges the two instead of silently replacing it. Press `Esc` at any time to stop a transfer after the current file.

## 💪 Why fishez?

### Speed First
Written in Rust with zero-copy rendering. Directory listings appear instantly. Preview large files without lag. Your terminal stays responsive.

### Developer-Focused
- **Syntax highlighting** in quick view for code files
- **One-key VS Code integration** (`F4`)
- **ripgrep-powered search** finds content across thousands of files in seconds
- **fd integration** for blazing-fast fuzzy file search
- **Raw image previews** via the `jpgfromrawlib`-supported camera formats directly in quick view

### Clean Architecture
Built with maintainability in mind using Clean Architecture principles. Domain logic stays pure, adapters are swappable, and the codebase remains readable. Contributing is straightforward.

### Cross-Platform
Works on macOS, Linux, and Windows. Uses native system commands for opening files and integrates with your existing toolchain.

## 🥊 vs. The Competition

| Feature | fishez | ranger | nnn | lf |
|---------|--------|--------|-----|-----|
| **Startup time** | ⚡ Instant | Slow (Python) | Fast | Fast |
| **Built-in preview** | ✅ Syntax highlighted | ✅ | ❌ External | ❌ External |
| **VS Code integration** | ✅ One key | ❌ Config needed | ❌ Config needed | ❌ Config needed |
| **ripgrep search** | ✅ Built-in | ❌ | ❌ | ❌ |
| **fd search** | ✅ Built-in | ❌ | ❌ | ❌ |
| **Zero config** | ✅ | ❌ | ⚠️ Minimal | ❌ |
| **Image preview** | ✅ iTerm2/kitty | ✅ | ❌ | ❌ |
| **Memory footprint** | ~5MB | ~50MB+ | ~3MB | ~4MB |

## 🏆 The Unfair Advantage

**fishez is built for the modern developer workflow.**

While other file managers try to be general-purpose, fishez optimizes for one thing: getting developers to their files faster. The combination of instant preview with syntax highlighting, one-key VS Code opening, and ripgrep/fd integration means you spend less time navigating and more time coding.

No rc files to configure. No plugins to install. No learning curve. It just works.

## 📦 Requirements

fishez works standalone but shines with these tools installed:

- **[fd](https://github.com/sharkdp/fd)** — Fast file search (`Ctrl+F`)
- **[ripgrep](https://github.com/BurntSushi/ripgrep)** — Content search (`Ctrl+R`)
- **VS Code CLI** — Editor integration (`F4`)

```bash
# macOS
brew install fd ripgrep

# Ubuntu/Debian
sudo apt install fd-find ripgrep

# Arch
sudo pacman -S fd ripgrep
```

## ⌨️ All Shortcuts

```
Navigation                    File Operations
─────────────────────────     ─────────────────────────
↑/↓         Move cursor       F5          Copy selected
Home/End    Jump to start/end F6          Move selected
Tab         Switch pane       Shift+F6    Rename
                              F7          New folder
Quick View (F3)               F8/Ctrl+W   Delete selected
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
                              F4          Open in VS Code
                              Ctrl+F      fd file search
                              Ctrl+R      ripgrep search
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
