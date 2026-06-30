## 1. Install Pipeline

- [x] 1.1 Fix CI release upload to ship all 4 platform binaries
- [x] 1.2 Create install.sh at repo root
- [x] 1.3 Update README Quick Start with one-liner install

## 2. Onboarding Overlay

- [x] 2.1 Add `show_onboarding` field to AppState
- [x] 2.2 Render onboarding banner in the header area
- [x] 2.3 Wire auto-dismiss timer and any-key dismiss in event loop

## 3. Shell Command Execution

- [x] 3.1 Add shell command input mode to event loop state
- [x] 3.2 Handle `!` key in normal mode (enter shell mode)
- [x] 3.3 Handle typing, backspace, enter, and esc in shell mode
- [x] 3.4 Implement `$1` and `$@` path substitution
- [x] 3.5 Execute command via `sh -c` and display output in QuickView
- [x] 3.6 Add command history with arrow-up cycling
