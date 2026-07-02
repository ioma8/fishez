# Onboarding Rework — first-run only, never blocking

Goal: the welcome card shows exactly once (first launch ever), waits for a key instead
of a timer, and the dismissing key passes through. Every later launch starts instantly.

## 1. Marker file in `~/.fishez/`

New module or extend `src/infrastructure/favorites_adapter.rs` (it already has
`home_dir()` — rename file to `config_adapter.rs` only if trivial, otherwise just add):

```rust
const ONBOARDED_FILE: &str = "onboarded";

pub fn is_onboarded() -> bool {
    home_dir().map_or(true, |h| h.join(CONFIG_DIR).join(ONBOARDED_FILE).exists())
}

pub fn mark_onboarded() {
    if let Some(h) = home_dir() {
        let dir = h.join(CONFIG_DIR);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(ONBOARDED_FILE), "");
    }
}
```

`is_onboarded()` defaults to `true` when `HOME` is missing — never nag in broken envs.
Export both from `infrastructure/mod.rs`.

## 2. Wire into AppState

`src/application/state.rs:198` hardcodes `show_onboarding: true`. Application layer must
not touch the filesystem — set it from the composition root instead:

- `AppState::new`: keep `show_onboarding: true` (existing tests rely on it).
- `src/main.rs`, after constructing state:
  `state.show_onboarding = !infrastructure::is_onboarded();`

## 3. Event loop: remove the timer, pass the key through

`src/presentation/event_loop.rs`:

- Delete the auto-dismiss block at the top of the loop
  (`if app_state.show_onboarding && onboarding_start.elapsed() > 2s { … }`, lines ~53-72)
  and the now-unused `onboarding_start` / `Instant` import.
- Replace the swallow-and-continue block (lines ~76-96) with dismiss-and-fall-through:

```rust
if app_state.show_onboarding {
    app_state.show_onboarding = false;
    infrastructure::mark_onboarded();
    redraw_current_view(/* unchanged args */);
    // no `continue` — the key falls through to quit check + route_input
}
```

Note: quit keys (Esc/F10/Ctrl+C) now also dismiss-then-quit — fine. `mark_onboarded`
is called on dismiss, not on show, so a user who launches and kills the terminal
still gets the card next time. Acceptable either way; dismiss is the simpler read.

- Mouse events do not dismiss the card (unchanged behavior, not worth the plumbing).

## 4. Onboarding card text

`renderer.rs` `draw_onboarding_banner`: change the hint row
`"press any key to start"` → `"press any key — shown only once"`. Width math is
computed from char counts, no other change needed.

## 5. demo.tape

The recording machine will have the marker, so the tape must fake a fresh HOME.
Add to the hidden preamble (before launching):

```
Type `export HOME=$(mktemp -d)`
Enter
```

This also empties favorites for the recording — fine. Because dismissal now requires a
key (no 2s auto-hide), the tape's post-launch `Sleep 2.6s` still works: the first
letter of the filter (`s` of "src") dismisses the card AND starts filtering — which
demos the pass-through nicely. Shorten that sleep to ~2s.

Re-record: `eval "$(TMPDIR=/tmp bash scripts/demo_setup.sh)" && FISHEZ_BIN=/tmp/fishez_target/release/fishez vhs demo.tape`.

## 6. Tests

- `infrastructure`: round-trip test with `HomeGuard` (exists in `test_support.rs`):
  fresh HOME → `is_onboarded() == false`; after `mark_onboarded()` → `true`, and
  `~/.fishez/onboarded` exists.
- `event_loop`: existing onboarding tests that assume the 2s timer must be updated or
  deleted. Add a unit test for the fall-through if the dismiss logic is extracted into
  a testable helper (`fn dismiss_onboarding(state) -> bool`); otherwise cover via the
  input-handler path.
- Renderer tests asserting banner content: update the hint-row string.

## Out of scope

No config option to re-show the card (`rm ~/.fishez/onboarded` is the escape hatch —
mention it nowhere, it's discoverable by the kind of person who wants it). No mouse
dismissal. No version-gated re-onboarding.
