## Context

fishez is a ~4700-line Rust TUI file manager with Clean Architecture. It already has CI building 4 platform binaries, a terminal renderer using crossterm, and an event loop with modal input modes (find, ripgrep, favorites, delete confirmation). The three quick-wins are independent and don't share state.

## Goals / Non-Goals

**Goals:**
- Ship prebuilt binaries for all 4 platforms to GitHub Releases
- One-line install via `curl | sh` and `cargo install`
- Show essential shortcuts to new users for 2 seconds on startup
- Let users press `!` to run shell commands with `$1`/`$@` path substitution
- Reuse existing QuickView::Text for command output — zero new rendering code

**Non-Goals:**
- Not adding any new dependencies
- No persistence for onboarding state (auto-dismiss always wins)
- No async command execution (sync is fine for quick commands)
- No shell command history persistence (in-memory only, YAGNI)

## Decisions

### 1. Onboarding: no persistence file, just auto-dismiss timer
Show the banner every startup; it auto-dismisses after 2 seconds and immediately on any keypress. Experienced users press a key within milliseconds, so they never see it. Zero file I/O, zero state management, zero edge cases.

### 2. Shell output: reuse QuickViewMode::Text
Instead of building a new output overlay, capture stdout+stderr, split into lines, and set `panel.mode = PanelMode::QuickView(QuickViewMode::Text { lines, start: 0 })`. All scrolling, dismissing, and rendering comes free.

### 3. Shell execution: sync, not async
Quick commands (`git log`, `ls`, `du -sh`) finish in <100ms. Async would require a loading spinner, channel messages, and complexity. Sync is simpler and correct for this use case. If someone reports `!make` freezing, add async then.

### 4. `$1` / `$@` substitution: simple string replacement
Not a full shell variable parser — just `str::replace("$1", ...)` and `str::replace("$@", ...)`. Multi-selected paths expand as `"$path1" "$path2" ...`. Single-selected is just `"$path"`. No edge cases for escaped `$1` in strings (that's what `sh -c` quoting handles).

### 5. CI fix: remove one condition
The release upload step guards on `matrix.os == 'ubuntu-latest'`, so only linux-x86_64 ships. Removing the guard lets all 4 matrix entries upload to the release.

### 6. install.sh: standalone, not a build tool
Just `curl + tar + mv`. No dependencies, no package manager, no Rust toolchain needed. Runs on macOS and Linux.

## Risks / Trade-offs

- **[Sync execution blocks UI]** → Commands finish fast for typical use. If blocking becomes an issue, wrap in a thread and use the existing mpsc channel pattern.
- **[install.sh needs updating on new platforms]** → The CI matrix defines what's built; install.sh just has the same platform list. Keep them in sync during CI changes.
- **[`!rm -rf $1` is dangerous]** → That's the shell, not fishez. The user types the command; fishez just runs it. Same as typing it in a terminal.
- **[Onboarding shows every startup]** → Only visible for 2 seconds max, and any keypress dismisses it. Invisible to power users.
