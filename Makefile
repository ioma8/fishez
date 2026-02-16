.PHONY: help build clean test check fmt lint install release-dev release

# Default target
help:
	@echo "Fishez Build & Release Scripts"
	@echo ""
	@echo "Available targets:"
	@echo "  build           - Build project in debug mode"
	@echo "  release         - Build optimized release version"
	@echo "  release-dev     - Build debug release version"
	@echo "  check           - Run cargo check (type checking)"
	@echo "  test            - Run all tests"
	@echo "  test-integration - Run integration tests only"
	@echo "  test-unit       - Run unit tests only"
	@echo "  fmt             - Format code with rustfmt"
	@echo "  fmt-check       - Check code formatting"
	@echo "  clippy          - Run clippy linter"
	@echo "  tree-check      - Validate dependency tree with cargo tree"
	@echo "  docs            - Build and open API documentation"
	@echo "  doc-html        - Build HTML documentation"
	@echo "  doc-check       - Check if docs build without errors"
	@echo "  install         - Install fishez binary"
	@echo "  uninstall       - Uninstall fishez binary"
	@echo "  release-ci      - Run all CI checks"
	@echo "  clean           - Clean build artifacts"
	@echo "  clean-all       - Clean all artifacts including cache"
	@echo "  update          - Update dependencies"
	@echo "  outdated        - Check for outdated dependencies"

# Build project
build:
	cargo build

# Build optimized release version
release:
	cargo build --release

# Build debug release version
release-dev:
	cargo build --release

# Run cargo check
check:
	cargo check

# Run all tests
test:
	cargo test

# Run integration tests
test-integration:
	cargo test --test integration_flows

# Run unit tests
test-unit:
	cargo test --lib

# Format code
fmt:
	cargo fmt

# Check code formatting
fmt-check:
	cargo fmt -- --check

# Run clippy linter
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Validate dependency tree
tree-check:
	cargo tree

# Build and open documentation
docs:
	cargo doc --no-deps --open

# Build HTML documentation
doc-html:
	cargo doc --no-deps --document-private-items

# Check if docs build without errors
doc-check:
	cargo doc --no-deps --document-private-items --quiet

# Install fishez binary
install:
	cargo install --path .

# Uninstall fishez binary
uninstall:
	cargo install --path . --uninstall

# Run all CI checks
release-ci: tree-check fmt-check check clippy test doc-check
	@echo "✓ All CI checks passed!"

# Clean build artifacts
clean:
	cargo clean

# Clean all artifacts including cache
clean-all:
	cargo clean
	rm -f favorites.txt err.txt log.txt
	rm -rf target/

# Update dependencies
update:
	cargo update

# Check for outdated dependencies
outdated:
	cargo outdated
