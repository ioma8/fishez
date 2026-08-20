#!/bin/bash
# Update the homebrew tap formula (ioma8/homebrew-tap) to a released fishez version.
# Usage: ./scripts/update-tap.sh [version]   (default: 0.4.0)
# Prereq: the GitHub release v<version> with binaries must already exist.

set -euo pipefail

VERSION=${1:-0.4.0}
TAP_REPO="ioma8/homebrew-tap"
ASSETS=(fishez-macos-aarch64 fishez-macos-x86_64 fishez-linux-aarch64 fishez-linux-x86_64)

# Fetch authoritative sha256 digests straight from the GitHub API (not curl:
# release downloads race CDN caching / in-flight uploads and served stale bytes once).
# Fails early if the release or any asset is missing.
SHAS=()
for a in "${ASSETS[@]}"; do
    SHAS+=($(gh release view "v$VERSION" --repo ioma8/fishez \
        --json assets --jq ".assets[] | select(.name == \"$a\") | .digest" \
        | sed 's/^sha256://'))
done

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

gh repo clone "$TAP_REPO" "$TMP/tap" -- --quiet
cd "$TMP/tap"

python3 - "$VERSION" "${SHAS[@]}" <<'PY'
import re, sys

version, shas = sys.argv[1], sys.argv[2:]
path = "Formula/fishez.rb"
s = open(path).read()
s = re.sub(r'version "\d+\.\d+\.\d+"', f'version "{version}"', s, count=1)
it = iter(shas)
s = re.sub(r'sha256 "[0-9a-f]{64}"', lambda m: f'sha256 "{next(it)}"', s)
open(path, "w").write(s)
PY

# Self-check: formula shas must equal the API digests, in order.
GOT=$(grep -oE 'sha256 "[0-9a-f]{64}"' Formula/fishez.rb | grep -oE '[0-9a-f]{64}')
WANT=$(printf '%s\n' "${SHAS[@]}")
if [ "$GOT" != "$WANT" ]; then
    echo "Error: formula shas do not match the release:" >&2
    paste <(echo "$GOT") <(echo "$WANT") >&2
    exit 1
fi

git add Formula/fishez.rb
git commit -q -m "Update fishez to $VERSION"
git push -q origin main

echo "✓ ioma8/tap/fishez updated to $VERSION (sha256s verified against GitHub API)"
echo "  brew install ioma8/tap/fishez"
