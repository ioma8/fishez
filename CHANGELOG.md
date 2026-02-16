# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release of Fishez terminal file manager
- Keyboard-driven terminal interface
- Quick view with syntax highlighting
- Multi-select functionality
- Delete files to trash
- Open files in VS Code with F4
- Fuzzy search with fd (F6)
- Content search with ripgrep (F7)
- Favorites system (Ctrl+D)
- Dual-pane mode support

### Features
- Two-pane mode for comparing directories
- Image preview in supported terminals (iTerm2, kitty)
- Directory creation and renaming (placeholder)
- Command palette (placeholder)
- Batch operations panel (placeholder)
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
- Command palette framework
- Batch operations panel framework

### Dependencies
- crossterm 0.28.1 - Terminal UI
- clipboard 0.5.0 - Clipboard operations
- trash 5.2.0 - Safe file deletion
- image 0.25.5 - Image preview
- textwrap 0.16.2 - Text wrapping
- tree_magic_mini 3.1.6 - File type detection

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

### Upcoming
- **v0.2.0** - Planned improvements
  - Directory creation/rename
  - Command palette
  - Batch operations
  - Image preview improvements
  - Better error handling
  - Config file support

### Known Limitations
- Image preview requires iTerm2 or kitty terminal
- External dependencies required: fd, ripgrep, VS Code CLI
- No command palette implementation yet
- No batch operations UI yet
- No directory creation UI yet

---

## Notes

- All shortcuts are documented in README.md
- CI runs on push and pull requests
- Uses semantic versioning
- MIT licensed

[Unreleased]: https://github.com/ioma8/fishez/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ioma8/fishez/releases/tag/v0.1.0
