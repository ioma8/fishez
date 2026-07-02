#!/bin/sh
# install.sh — one-line install for fishez
# Usage: curl -sfL https://raw.githubusercontent.com/ioma8/fishez/main/install.sh | sh
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
    echo "Then run: fishez --install-shell"
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64)  ARCH="x86_64" ;;
  aarch64|arm64) ARCH="aarch64" ;;
  *)
    echo "Unsupported architecture: $ARCH"
    echo "Try: cargo install fishez"
    echo "Then run: fishez --install-shell"
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

/usr/local/bin/$BIN --install-shell || {
  echo "To enable fz later, run: fishez --install-shell"
}

echo "✓ fishez installed. Run 'fishez' to start, or 'fz' to cd on exit."
