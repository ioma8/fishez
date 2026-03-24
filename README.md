<div align="center">

<img src="Fishez_logo.svg" alt="fishez logo" width="180" />

# fishez

**A lightning-fast terminal file manager built for developers**

[![CI](https://github.com/ioma8/fishez/actions/workflows/ci.yml/badge.svg)](https://github.com/ioma8/fishez/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org/)

Navigate files at the speed of thought. Preview code with syntax highlighting. Search instantly with ripgrep. Open in VS Code with one keystroke.

</div>

---

## ✨ What is fishez?

fishez is a keyboard-driven terminal file manager designed for developers who live in the terminal. It combines the speed of command-line navigation with the convenience of visual file browsing — without ever leaving your workflow.

**No mouse required. No config files. Just install and go.**

## 🚀 Quick Start

```bash
# Clone and run
git clone https://github.com/ioma8/fishez.git
cd fishez
cargo run

# Or with dual-pane mode
cargo run -- -2
```

## 🎯 Core Features

| Feature | Shortcut | Description |
|---------|----------|-------------|
| **Navigate** | `↑↓` / `Enter` / `Backspace` | Move through directories instantly |
| **Quick View** | `F3` | Preview files with syntax highlighting |
| **Multi-select** | `Space` | Batch operations on multiple files |
| **Delete** | `Ctrl+W` | Move to trash (safe delete) |
| **Open in VS Code** | `F4` | Jump straight into your editor |
| **Fuzzy Search** | `F6` | Find files with `fd` |
| **Content Search** | `F7` | Search file contents with `ripgrep` |
| **Favorites** | `Ctrl+D` | Quick-jump to pinned directories |
| **Filter** | *Start typing* | Instantly filter current directory |
| **Help** | `F1` | Show all shortcuts |

### Dual-Pane Mode

Run with `-2` or `--two-pane` for a split view. Press `Tab` to switch focus between panels — perfect for comparing directories or moving files.

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

- **[fd](https://github.com/sharkdp/fd)** — Fast file search (`F6`)
- **[ripgrep](https://github.com/BurntSushi/ripgrep)** — Content search (`F7`)
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
Navigation                    Actions
─────────────────────────     ─────────────────────────
↑/↓         Move cursor       Enter       Open / Enter dir
Home/End    Jump to start/end Backspace   Go up one level
Tab         Switch pane       Space       Toggle selection
                              Ctrl+W      Delete selected
Quick View (F3)               Ctrl+Enter  Copy path
─────────────────────────     Ctrl+Shift+Enter  Copy absolute path
↑/↓         Scroll content
←/→         Prev/next file    Search & Tools
PgUp/PgDn   Page scroll       ─────────────────────────
Esc         Close preview     F4          Open in VS Code
                              F6          fd file search
Favorites (Ctrl+D)            F7          ripgrep search
─────────────────────────     F1          Show help
↑/↓         Navigate          F10/Ctrl+C  Quit
Enter       Jump to favorite
Ctrl+Shift+D  Add current dir
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
