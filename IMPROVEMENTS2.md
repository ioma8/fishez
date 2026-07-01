# fishez — Approachability Improvements (Cuts, Not Extensions)

Second-pass assessment focused on removing friction rather than adding features. These are things already in the app that make it *less* approachable, ranked by payoff. Complements `IMPROVEMENTS.md`.

## 1. Retire the `--two-pane` flag as a concept — Ctrl+T already does it

The code has a runtime toggle (`shortcuts.rs:198`), yet the README's quick-start teaches `fishez --two-pane` as if it were a launch-time decision. That's a whole CLI concept users must learn for something one keystroke does live. Lead the README with "press Ctrl+T for dual pane"; keep the flag as an undocumented alias or delete it. Zero code added, one concept removed.

## 2. Replace the `clipboard` crate with OSC 52 escape codes

`clipboard 0.5.0` is unmaintained since ~2019 and drags X11 build dependencies on Linux — meaning `cargo install fishez` can *fail to compile* on Wayland or headless boxes, a first-contact death for a chunk of the audience. It's used for one feature (copy path). OSC 52 is ~10 lines, zero dependencies, and works over SSH, which the X11 clipboard never did. A dependency cut that also makes the feature better.

## 3. Trim tokio to `features = ["rt"]`

Cargo.toml asks for `rt-multi-thread` + `macros`, but the only tokio use in the codebase is a `current_thread` runtime to `block_on` the RAW-image library (`raw_image.rs:48`). The declared features are pure compile-time and binary-size cost. Full removal is only possible if `jpgfromrawlib` grows a sync API — not worth chasing.

## 4. Stop writing `err.txt` into the user's current directory

The panic hook (`main.rs:98`) appends to `./err.txt` — a crash while browsing someone's project litters *their* repo with a log file. `std::env::temp_dir().join("fishez-panic.log")` is the same line count. Also drop the dead `ctrlc` dependency (declared in Cargo.toml, never used in src) while in there.

## 5. Clean the repo's front page

`009.jpg` (a stray photo) sits in the root, alongside `CLEAN_ARCH.md`, `MULTIPLATFORM_BUILD.md`, `RELEASE_GUIDE.md`. For a project whose growth channel is people landing on the GitHub page, the root listing is UI. Delete the jpg, move dev docs into `docs/`. First screen should be: logo, demo GIF, install command.

## 6. Esc quits when there's nothing left to cancel

Today quitting requires F10 or Ctrl+C — F10 is frequently swallowed by macOS/terminal emulators, and Ctrl+C-to-exit reads as "I killed it" rather than "I quit it". Esc already cancels overlays and filters; letting it exit from a clean normal state gives the "how do I get out?!" user (the most common TUI first experience) an instinctive answer. Small addition, but it removes a wall rather than adding a feature.

## 7. `fishez <path>` to start in a directory

Currently it only opens in cwd (`state.rs:71`). One line in main. People will try `fishez ~/projects` in their first minute; it should just work.

## Verified fine — do not touch

The footer already shows contextual action hints (`draw_footer_actions`), mouse capture is genuinely used (click, scroll, context menu — not dead weight), and an onboarding overlay exists. The identity — no config file, keyboard-first, type-to-filter — is the product; none of these cuts touch it.
