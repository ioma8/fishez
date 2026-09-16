#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUT="$ROOT/_site"
rm -rf "$OUT"
mkdir -p "$OUT/assets"
cp "$ROOT/site/index.html" "$ROOT/site/styles.css" "$ROOT/site/script.js" "$ROOT/site/sitemap.xml" "$ROOT/site/social-preview.png" "$OUT/"
cp "$ROOT/Fishez_logo.svg" "$ROOT/demo.gif" "$ROOT/site/assets/demo-poster.png" "$OUT/assets/"
printf '%s\n' "Staged site in $OUT"
