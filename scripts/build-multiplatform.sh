# Multi-platform build script for Fishez
# This script builds binaries for Linux and macOS on various architectures

#!/bin/bash
set -e

VERSION=${1:-$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')}
BUILD_DIR="target/release"
BUILD_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
BUILD_COMMIT=$(git rev-parse --short HEAD)

# Supported platforms
declare -A PLATFORMS=(
    ["linux-x86_64"]="x86_64-unknown-linux-gnu"
    ["linux-aarch64"]="aarch64-unknown-linux-gnu"
    ["macos-x86_64"]="x86_64-apple-darwin"
    ["macos-aarch64"]="aarch64-apple-darwin"
)

echo "🏗️  Building Fishez $VERSION for multiple platforms..."
echo ""

# Build for all platforms
for arch in "${!PLATFORMS[@]}"; do
    GOOS=${arch%%-*}
    GOARCH=${arch##*-}

    echo "🔨 Building for $arch ($GOOS/$GOARCH)..."

    # Install cross compiler if needed
    if ! command -v cross &> /dev/null; then
        echo "Installing cross compiler..."
        cargo install cross --git https://github.com/cross-rs/cross
    fi

    # Build with cross
    cross build --release --target "${PLATFORMS[$arch]}"
done

echo ""
echo "✅ Build complete!"
echo ""
echo "Built binaries:"
ls -lh target/release/fishez

echo ""
echo "📋 Build info:"
echo "  Version: $VERSION"
echo "  Build time: $BUILD_TIME"
echo "  Commit: $BUILD_COMMIT"
