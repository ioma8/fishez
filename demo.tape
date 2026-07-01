# fishez README demo. Record with:
#   cargo build --release
#   eval "$(scripts/demo_setup.sh)"
#   FISHEZ_BIN="$PWD/target/release/fishez" vhs demo.tape
# Requires vhs + ttyd. vhs cannot send F-keys, so the tape uses the
# Ctrl aliases (Ctrl+P quick view, Ctrl+Y copy).

Output demo.gif

Set Shell bash
Set FontSize 18
Set Width 1200
Set Height 700
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 60ms

Hide
Type `alias fishez="${FISHEZ_BIN:-fishez}"`
Enter
Type "cd $FISHEZ_DEMO_DIR && clear"
Enter
Show

# --- Launch: onboarding banner shows for ~2s ---
Type "fishez"
Sleep 500ms
Enter
Sleep 3s
Down
Sleep 500ms

# --- Type-to-filter, jump into src/ ---
Type "src"
Sleep 1.2s
Enter
Sleep 1s

# --- Quick view with highlighting, arrow through files ---
Down
Sleep 500ms
Ctrl+P
Sleep 2.2s
Right
Sleep 1.8s
Ctrl+P
Sleep 700ms

# --- Dual pane, point right pane at backup/ ---
Ctrl+T
Sleep 1.5s
Tab
Sleep 700ms
Type "backup"
Sleep 800ms
Enter
Sleep 800ms
Tab
Sleep 700ms

# --- Back to project root, into assets/, select both big files ---
Backspace
Sleep 800ms
Type "assets"
Sleep 800ms
Enter
Sleep 800ms
Down
Sleep 400ms
Space
Sleep 500ms
Down
Sleep 400ms
Space
Sleep 900ms

# --- Copy 1.4 GB to the other pane with live progress ---
Ctrl+Y
Sleep 1.5s
Enter
Sleep 4.5s

# --- Hold the final frame ---
Sleep 2.5s
