## Why

fishez has a solid terminal file manager, but it's held back by three things: install friction, zero discovery for new users, and no way to run shell commands from within the TUI. These are all quick, independent wins that each remove a reason for someone to bounce off or not adopt fishez.

## What Changes

1. **Install pipeline** — fix the CI release workflow so all 4 platform binaries ship to GitHub Releases, add a one-line install script (`install.sh`), and publish to crates.io so `cargo install fishez` works.

2. **Startup onboarding overlay** — show a compact help banner on first launch that auto-dismisses after 2 seconds or on any keypress, so new users see essential shortcuts without needing to know F1 exists.

3. **Shell command execution** — press `!` to type a shell command from within the TUI, with `$1` / `$@` expanding to the current file/dir path(s). Output renders in the existing QuickView text display. History via arrow-up (in-memory, no persistence).

## Capabilities

### New Capabilities
- `install-pipeline`: One-line install script, CI release hardening, crates.io publish.
- `onboarding-tutorial`: First-run overlay that auto-dismisses, zero persistence.
- `shell-command`: Shell command input from within the TUI with path substitution and output display.

### Modified Capabilities
- *(none — all capabilities are new)*

## Impact

- `.github/workflows/ci.yml` — one character change (remove platform guard on release upload)
- `src/application/state.rs` — add `show_onboarding: bool` field
- `src/presentation/event_loop.rs` — route `!` key, track timer for onboarding dismiss
- `src/presentation/input_handler.rs` — handle shell command input mode
- `src/presentation/terminal/renderer.rs` — render onboarding banner in header
- New file: `install.sh` at repo root
- `README.md` — update Quick Start section
