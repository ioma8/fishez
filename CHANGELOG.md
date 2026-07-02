# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `fz` cd-on-exit shell wrapper via `fishez --init`
- Start path support, e.g. `fishez ~/projects`
- Safe background copy/move with progress and overwrite prompts
- Shell command overlay with `{1}` for the focused path and `{@}` for selected paths
- OSC52 path copy for terminal clipboard workflows
- Hidden-file toggle with `Ctrl+H`
- Optional `bat` integration for richer text previews

### Features
- Keyboard-driven terminal file manager
- Quick view for text, directories, and terminal-supported images
- Open files in `$EDITOR` with F4, falling back to VS Code
- Fuzzy search with fd (Ctrl+F)
- Content search with ripgrep (Ctrl+R)
- Favorites system (Ctrl+D), stored in `~/.fishez/favorites.txt`
- Two-pane mode for comparing directories
- Directory creation, rename, trash delete, copy, and move operations
- Structured logging infrastructure
- Clean Architecture implementation

## [0.1.0] - 2024-02-16

### Added
- Terminal UI framework using crossterm
- File system abstraction layer
- Navigation system with cursor movement
- Multi-selection for batch operations
- Trash integration for safe file deletion
- Clipboard integration
- VS Code CLI adapter
- fd integration for fuzzy file search
- ripgrep integration for content search
- Favorites management system

### Dependencies
- crossterm 0.28.1 - Terminal UI
- trash 5.2.0 - Safe file deletion
- image 0.25.5 - Image preview
- textwrap 0.16.2 - Text wrapping

### Architecture
- Clean Architecture layers (domain, application, infrastructure, presentation)
- Dependency inversion principle
- Composition root pattern
- Event-driven UI architecture

### Documentation
- README with feature overview
- CLEAN_ARCH.md with architecture documentation
- FLOW.md with manual test steps
- AGENTS.md with contribution guidelines

---

## Version History

### Current Notes
- Image preview works best in iTerm2 or kitty terminals.
- `fd`, `ripgrep`, `bat`, and the VS Code CLI are optional external tools.

---

## Notes

- All shortcuts are documented in README.md
- CI runs on push and pull requests
- Uses semantic versioning
- MIT licensed

[Unreleased]: https://github.com/ioma8/fishez/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ioma8/fishez/releases/tag/v0.1.0
