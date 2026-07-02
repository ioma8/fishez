#!/bin/sh
# install.sh — one-line install for fishez
# Usage: curl -sfL https://ioma8.github.io/fishez/install.sh | sh
set -eu

REPO="ioma8/fishez"
BIN="fishez"
VERSION="${1:-latest}"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  linux)  ;;
  darwin) OS="macos" ;;
  *)
    echo "Unsupported OS: $OS"
    echo "Try: cargo install fishez"
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64)  ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *)
    echo "Unsupported architecture: $ARCH"
    echo "Try: cargo install fishez"
    exit 1
    ;;
esac

PLATFORM="$OS-$ARCH"
URL="https://github.com/$REPO/releases/$VERSION/download/fishez-$PLATFORM"

echo "Downloading fishez for $PLATFORM..."

if command -v curl >/dev/null 2>&1; then
  curl -sfLo /tmp/fishez "$URL"
elif command -v wget >/dev/null 2>&1; then
  wget -qO /tmp/fishez "$URL"
else
  echo "Need curl or wget"
  exit 1
fi

chmod +x /tmp/fishez

if [ "$(id -u)" -eq 0 ]; then
  mv /tmp/fishez /usr/local/bin/$BIN
else
  echo "Installing to /usr/local/bin (may ask for password)..."
  sudo mv /tmp/fishez /usr/local/bin/$BIN
fi

# Install the fz() shell wrapper (cd to last viewed dir on exit)
case "${SHELL:-}" in
  */zsh)  RC="$HOME/.zshrc" ;;
  */bash) RC="$HOME/.bashrc" ;;
  *)      RC="" ;;
esac

FZ_LINE='eval "$(fishez --init)"  # fz: fishez wrapper that cds to the last viewed dir'
if [ -n "$RC" ]; then
  if ! grep -q "fishez --init" "$RC" 2>/dev/null; then
    printf '\n%s\n' "$FZ_LINE" >> "$RC"
    echo "✓ Added fz shell function to $RC (open a new shell to use it)."
  fi
else
  echo "To get the fz function (cd on exit), add this to your shell rc:"
  echo "  $FZ_LINE"
fi

echo "✓ fishez installed. Run 'fishez' to start, or 'fz' to cd on exit."
