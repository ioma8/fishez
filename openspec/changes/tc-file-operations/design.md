## Context

fishez already has a working delete flow (Ctrl+W), a modal overlay system, an async message channel (`Sender<Message>`), and a `FileSystemPort` trait backed by `StdFileSystem`. The event loop threads modal state (`delete_paths`, `find_filter`, etc.) as `Option<T>` through `run` → `route_input` → `handle_modal_overlays`. New operations follow the same pattern.

**Key conflict resolution required:** TC conventions assign F6 (move) and F7 (new folder) to keys that fishez currently uses for fd find and ripgrep. These must be relocated before the TC bindings can be added.

## Goals / Non-Goals

**Goals:**
- F5 copy, F6 move, Shift+F6 rename, F7 new folder, F8 delete — all working
- Dual-pane aware: F5/F6 pre-fill the opposite pane's path
- Ctrl+W alias preserved
- No new dependencies

**Non-Goals:**
- Progress bar for large copies (follow-up)
- Overwrite prompting when destination exists (first pass: error notification)
- Recursive directory size calculation before copy
- Undo/redo of file operations

## Decisions

### D1: Relocate fd find (F6 → Alt+F7) and ripgrep (F7 → Ctrl+G)

TC users have deep muscle memory for F6=move and F7=new folder. Keeping fd/ripgrep on those keys would make fishez feel broken to the target audience.

**Alt+F7** is the TC convention for "Find Files" — a natural home for fd find.
**Ctrl+G** is the grep mnemonic and is otherwise unused — a natural home for ripgrep.

All existing help text, README, and footer actions updated accordingly.

**Alternatives considered:**
- F9/F10 for fd/ripgrep — F10 is quit, F9 is free but arbitrary and hard to remember.
- Ctrl+F / Ctrl+R — reasonable but Alt+F7 is more discoverable for TC users.

### D2: Reuse the existing modal overlay pattern for copy/move/rename/new-folder prompts

All existing modals are `Option<T>` fields threaded through the event loop. Four new fields:
```rust
rename_input: Option<String>
new_folder_input: Option<String>
copy_dest: Option<CopyMoveState>
move_dest: Option<CopyMoveState>
```
```rust
struct CopyMoveState {
    sources: Vec<PathBuf>,
    dest: String,  // editable destination string
}
```

**Alternatives considered:** A single `ActiveModal` enum — cleaner long-term but requires refactoring all existing modals; deferred.

### D3: F6 = move, Shift+F6 = rename (macOS TC convention)

In classic TC, F6 is "Rename or Move" — a combined dialog. On macOS, the convention splits into F6 (move) and Shift+F6 (rename). This matches how macOS TC-compatible tools typically work and avoids a combined dialog that would complicate the UX.

### D4: std::fs::rename with copy+delete fallback for move

`std::fs::rename` is atomic and instant on the same filesystem. For cross-device moves, fall back to recursive copy + delete. Do NOT delete source until copy succeeds.

### D5: Copy/move destination pre-fill

In dual-pane: `app_state.inactive_panel().current_path`. In single-pane: empty string. The prompt is always editable.

### D6: Synchronous first pass for all file operations

The shell command feature established the async pattern. File operations will be synchronous for the first pass (acceptable for typical file manager use); the async upgrade path is identical to what was done for shell commands.

## Risks / Trade-offs

- **Overwrite not handled** → Error notification if destination exists. Follow-up adds overwrite prompt.
- **Large directory copy blocks UI** → Sync first pass; async upgrade is straightforward.
- **Four more `Option<T>` fields in the event loop** → The argument list is already suppressed with `too_many_arguments`. A modal enum refactor is the long-term fix.
- **Alt+F7 key detection** — crossterm must emit `Shift` modifier for Shift+F6 and `Alt` for Alt+F7; verify on macOS terminal emulators (iTerm2, Terminal.app) during testing.

## Migration Plan

No data migration needed. All changes are additive except the F6/F7 reassignment, which is a keybinding change (no stored state). Users relying on F6 for find or F7 for ripgrep must learn Alt+F7 / Ctrl+G. Document in README changelog.

## Open Questions

- Should F5/F6 operate when quick-view is active? **Proposed: no — require normal mode.**
- Overwrite prompt on destination conflict: error (first pass) or ask? **Deferred.**
- Does crossterm correctly emit `KeyModifiers::SHIFT` for Shift+F6 across all macOS terminals? **Verify during implementation.**
