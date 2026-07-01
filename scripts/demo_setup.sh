#!/bin/sh
# Stages the demo directory used by demo.tape (vhs).
# Usage: eval "$(scripts/demo_setup.sh)"  — prints the export for FISHEZ_DEMO_DIR.
set -e

D="${TMPDIR:-/tmp}/fishez-demo/awesome-project"
rm -rf "$D"
mkdir -p "$D/src" "$D/docs" "$D/assets" "$D/backup"

cat > "$D/src/main.rs" <<'EOF'
//! awesome-project entry point.
use std::collections::HashMap;

fn main() {
    let config = load_config();
    let mut scores: HashMap<String, u32> = HashMap::new();
    for (name, points) in config.iter() {
        scores.insert(name.clone(), points * 2);
    }
    println!("loaded {} entries", scores.len());
}

fn load_config() -> Vec<(String, u32)> {
    vec![
        ("alpha".into(), 10),
        ("beta".into(), 20),
        ("gamma".into(), 30),
    ]
}
EOF

cat > "$D/src/parser.rs" <<'EOF'
//! Tiny expression parser.
pub enum Token { Num(f64), Plus, Minus }

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut out = Vec::new();
    for word in input.split_whitespace() {
        match word {
            "+" => out.push(Token::Plus),
            "-" => out.push(Token::Minus),
            n => {
                if let Ok(v) = n.parse() {
                    out.push(Token::Num(v));
                }
            }
        }
    }
    out
}
EOF

printf '# awesome-project\n\nA demo project for the fishez screencast.\n' > "$D/README.md"
printf 'MIT License\n' > "$D/LICENSE"
printf '[package]\nname = "awesome-project"\nversion = "1.0.0"\n' > "$D/Cargo.toml"
printf 'Design notes live here.\n' > "$D/docs/architecture.md"
printf 'TODO: write the user guide.\n' > "$D/docs/guide.md"
printf 'old stuff\n' > "$D/backup/notes-2019.txt"
# Big files so the background-copy progress is actually visible in the recording.
mkfile -n 900m "$D/assets/big-video.mp4" 2>/dev/null || dd if=/dev/zero of="$D/assets/big-video.mp4" bs=1048576 count=900 2>/dev/null
mkfile -n 500m "$D/assets/photo-shoot.raw" 2>/dev/null || dd if=/dev/zero of="$D/assets/photo-shoot.raw" bs=1048576 count=500 2>/dev/null

echo "export FISHEZ_DEMO_DIR='$D'"
