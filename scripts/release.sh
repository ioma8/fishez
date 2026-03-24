#!/bin/bash
# Release script for Fishez
# Usage: ./scripts/release.sh <version>

set -e

VERSION=${1:-0.2.0}
PREV_VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

if [ "$PREV_VERSION" == "$VERSION" ]; then
    echo "Error: Version $VERSION is already set in Cargo.toml"
    exit 1
fi

echo "📦 Releasing Fishez version $VERSION"
echo "Previous version: $PREV_VERSION"
echo ""

# 1. Update version in Cargo.toml
echo "📝 Updating version in Cargo.toml..."
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
rm Cargo.toml.bak

# 2. Update CHANGELOG.md
echo "📝 Updating CHANGELOG.md..."
sed -i.bak "/## \[Unreleased\]/,/^## \[/ {
    /^## \[Unreleased\]/ { 
        N
        s/Unreleased\]\n\n### Added/Unreleased\]\n\n## [$VERSION] - $(date +%Y-%m-%d)\n### Added/
    }
    /^## \[/ {
        n
        s/\[.*\] - $(date +%Y-%m-%d)/[$VERSION] - $(date +%Y-%m-%d) - Upcoming/
    }
}" CHANGELOG.md
rm CHANGELOG.md.bak

# 3. Update version in CHANGELOG.md if not already done
if ! grep -q "\[$VERSION\]" CHANGELOG.md; then
    sed -i.bak "/## \[Unreleased\]/,/^## \[/ {
        /^## \[Unreleased\]/ {
            N
            s/Unreleased\]\n\n### Added/Unreleased\]\n\n## [$VERSION] - $(date +%Y-%m-%d)\n### Added/
        }
    }" CHANGELOG.md
    rm CHANGELOG.md.bak
fi

# 4. Create git tag
echo "🏷️  Creating git tag $VERSION..."
git tag -a "$VERSION" -m "Release fishez $VERSION"

# 5. Show changes
echo ""
echo "📋 Summary of changes:"
echo "  - Version updated: $PREV_VERSION -> $VERSION"
echo "  - CHANGELOG.md updated"
echo "  - Git tag created: $VERSION"

# 6. Show instructions for pushing
echo ""
echo "🚀 To complete the release, push the changes and tag:"
echo "  git push origin main"
echo "  git push origin $VERSION"

# 7. Verify release pipeline will run
echo ""
echo "✅ GitLab CI will automatically:"
echo "  - Build release binary"
echo "  - Run all tests"
echo "  - Generate documentation"
echo "  - Create release with binary artifacts"
echo "  - Create GitLab release page"
