# fishez README demo. Record with:
#   cargo build --release
#   eval "$(TMPDIR=/tmp bash scripts/demo_setup.sh)"   # short path keeps the header clean
#   FISHEZ_BIN=/tmp/fishez_target/release/fishez vhs demo.tape
# Requires vhs + ttyd, bat (highlighted previews), and edit (the editor demo:
# brew install edit). vhs cannot send F-keys, so the tape uses the Ctrl
# aliases (Ctrl+P quick view, Ctrl+O editor).

Output demo.gif

Set Shell bash
Set FontSize 20
Set Width 1200
Set Height 700
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 60ms

Hide
Type `export FISHEZ_BIN="${FISHEZ_BIN:-fishez}"`
Enter
Type `mkdir -p /tmp/fishez-demo-bin && ln -sf "$FISHEZ_BIN" /tmp/fishez-demo-bin/fishez && export PATH="/tmp/fishez-demo-bin:$PATH"`
Enter
Type `export HOME=$(mktemp -d)`
Enter
# NO_COLOR (inherited from the recording user's shell) makes crossterm strip
# all colors — the TUI records monochrome. TERM/COLORTERM are already correct.
Type "unset NO_COLOR; export BAT_THEME=TwoDark"
Enter
Type `eval "$($FISHEZ_BIN --init)"`
Enter
Type "export EDITOR=edit"
Enter
Type "clear"
Enter
Show

# --- Launch with fz; first typed key dismisses onboarding and filters ---
Type "fz $FISHEZ_DEMO_DIR"
Sleep 300ms
Enter
Sleep 2s

# --- Star feature 1: type-to-filter, letter by letter ---
Type@200ms "src"
Sleep 800ms
Enter
Sleep 600ms

# --- Star feature 2: quick view with syntax highlighting, flip files ---
Down
Sleep 400ms
Ctrl+P
Sleep 1.6s
Right
Sleep 1.6s
Ctrl+P
Sleep 500ms

# --- Search project contents with ripgrep ---
Ctrl+R
Sleep 500ms
Type "parse"
Sleep 700ms
Enter
Sleep 1s

# --- Open the match in $EDITOR (suspends the TUI), then quit back ---
Ctrl+O
Sleep 1.8s
Ctrl+Q
Sleep 800ms

# --- Quit; fz drops the shell in the browsed directory ---
Escape
Sleep 600ms
Type "pwd"
Sleep 300ms
Enter
Sleep 2s
