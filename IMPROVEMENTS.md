# fishez — Best-ROI Improvements

Assessment of what would improve the app most, ranked by return on effort. The codebase (~28k lines, clean architecture) is solid — background copy/move with overwrite prompts, trash delete, fd/rg search, image preview, quick view all work well. The gaps are missing table-stakes features and distribution polish, not architecture.

## 1. cd-on-exit shell integration (biggest single win)

Quitting fishez drops you back where you started. Every daily-driver terminal FM (yazi, nnn, lf) ships a tiny shell wrapper so quitting leaves your shell in the browsed directory — it's *the* feature that turns "nice demo" into "replaces cd for me". Implementation: a `--cwd-file <path>` flag that writes the final directory on exit, plus a 5-line `fz()` shell function in the README/install.sh.

## 2. Respect `$EDITOR`

F4 is hardcoded to VS Code (`src/infrastructure/open_adapter.rs`). The terminal-FM audience is heavily vim/helix/neovim — hardcoding VS Code alienates exactly the people who'd star and share this. Try `$EDITOR` first, fall back to `code`.

## 3. Hidden-files toggle and sort modes

No dotfile toggle and no sort-by-size/date anywhere (directory listings aren't even explicitly sorted — they come back in `read_dir` order). These are the first two things a new user reaches for; their absence reads as "unfinished". Two keybindings, a state flag, and a sort in `refresh_entries`.

## 4. Real syntax highlighting via `bat` shell-out

The README promises "syntax highlighting" but the implementation is a ~30-keyword string matcher (`quick_view.rs`). Lazy fix matching the existing fd/rg pattern: shell out to `bat --color=always --plain` when installed, keep the naive highlighter as fallback. Preview quality is what people screenshot.

## 5. Demo GIF + distribution — the viral lever

For TUI apps, virality is ~80% a gorgeous animated demo at the top of the README (make one with `vhs`, scripted and reproducible), plus one-command install people already trust: crates.io, a Homebrew tap, then post to r/rust, r/commandline, HN, and the awesome-tuis list. yazi and superfile grew on exactly this playbook.

## 6. Doc hygiene (30 minutes, high trust payoff)

README says fuzzy search is `Alt+F7` and content search `Ctrl+G`, but the code binds `Ctrl+F`/`Ctrl+R` (`shortcuts.rs:169-173`). CHANGELOG still lists "(placeholder)" features and stale bindings. Wrong docs on first contact kills adoption faster than a missing feature.

## What NOT to do

Mouse support, config files, themes, plugins, tabs, archive browsing. "No config, keyboard-only, fast" is the identity — adding those dilutes it and competes with yazi on yazi's turf. Items 1–3 are each a day or less; item 5 determines whether anyone ever sees 1–4.
