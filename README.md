# fishez

fishez is a fast terminal file manager with previews and extensible actions.

## Run
```
cargo run            # single pane
cargo run -- -2      # two panes (Tab switches focus)
```

## Core actions
- Arrows to move, Enter to open, Backspace to go up
- F3 quick view (text/images/dirs), Space to multi-select, Ctrl+W delete
- F4 open in VS Code, F6 fd search, F7 ripgrep, Ctrl+D favorites
- F1 help overlay, F10/Ctrl+C quit

## Notes
- External tools: `fd`, `ripgrep`, `xdg-open` (Linux), `code` (VS Code CLI)
- Favorites file: `favorites.txt`
