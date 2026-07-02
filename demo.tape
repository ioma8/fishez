# fishez README demo. Record with:
#   cargo build --release
#   eval "$(bash scripts/demo_setup.sh)"
#   FISHEZ_BIN=/tmp/fishez_target/release/fishez vhs demo.tape
# Requires vhs + ttyd. The tape sets EDITOR=true so the editor
# action is visible in-flow without launching an external application.

Output demo.gif

Set Shell bash
Set FontSize 18
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
Type "export TERM=xterm-256color COLORTERM=truecolor BAT_THEME=TwoDark"
Enter
Type "export EDITOR=true"
Enter
Type `eval "$($FISHEZ_BIN --init)"`
Enter
Type "clear"
Enter
Show

# --- Launch with fz into the staged project ---
Type "fz $FISHEZ_DEMO_DIR"
Sleep 500ms
Enter
Sleep 2.5s

# --- Navigate into source and preview code ---
Type "src"
Sleep 900ms
Enter
Sleep 900ms
Down
Sleep 500ms
Ctrl+P
Sleep 2s
Right
Sleep 1.4s
Ctrl+P
Sleep 500ms

# --- Search project contents with ripgrep ---
Ctrl+R
Sleep 700ms
Type "parse"
Sleep 1.2s
Enter
Sleep 1s
Escape
Sleep 500ms

# --- Open in editor with the Ctrl alias, then quit back to shell ---
Ctrl+O
Sleep 1.3s
Escape
Sleep 700ms

# --- Hold the final frame ---
Sleep 2.5s
