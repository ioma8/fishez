
# fishez

fishez is a terminal file manager written in Rust with extensible features and a modern UI.

## Main Features
- Navigate the file system in the terminal
- Fast filtering and file search (supports fd, ripgrep)
- Quick preview of text and image files
- Open files in default applications or in VS Code
- Favorites management
- Multiselect and batch operations (in progress)
- Delete files to trash

## Controls (selection)
- Arrows: move
- Enter: open file/folder
- F3: quick preview
- F4: open in VS Code
- F6: search (fd)
- F7: search (ripgrep)
- Ctrl+D: favorites
- Ctrl+W: delete
- F10/Ctrl+C: quit
- Tab: switch active pane (two-pane mode)
- Space: toggle multi-select highlight (delete uses the current selection)
- F1: show condensed help (full list)

## Dependencies
- Rust (2024 edition)
- [crossterm](https://crates.io/crates/crossterm)
- [clipboard](https://crates.io/crates/clipboard)
- [image](https://crates.io/crates/image)
- [iterm2img](https://crates.io/crates/iterm2img)
- [little_exif](https://crates.io/crates/little_exif)
- [textwrap](https://crates.io/crates/textwrap)
- [trash](https://crates.io/crates/trash)
- [tree_magic_mini](https://crates.io/crates/tree_magic_mini)

## Running

```bash
cargo run            # single-pane (default)
cargo run -- -2      # two-pane layout (alias: --two-pane)
```

## Notes
- Some features require external tools: `fd`, `ripgrep`, `xdg-open` (Linux), `code` (VS Code CLI)
- Favorites configuration file: `favorites.txt`

---

The project is under development. Contributors welcome!
