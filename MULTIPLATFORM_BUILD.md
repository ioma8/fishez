# Multi-Platform Build Guide

This guide explains how Fishez handles cross-platform builds and releases for Linux and macOS on x86_64 and ARM architectures.

## 🏗️ Supported Platforms

| Platform | Architecture | Target Triple | Description |
|----------|--------------|---------------|-------------|
| **Linux** | x86_64 | x86_64-unknown-linux-gnu | 64-bit Intel/AMD |
| **Linux** | ARM64 | aarch64-unknown-linux-gnu | 64-bit ARM (Apple Silicon, AWS Graviton, etc.) |
| **macOS** | x86_64 | x86_64-apple-darwin | Intel Macs |
| **macOS** | ARM64 | aarch64-apple-darwin | Apple Silicon (M1, M2, M3) |

## 📦 Build Process

### GitHub Actions CI/CD

The GitHub Actions workflow builds binaries in **parallel** across all platforms:

```yaml
matrix:
  include:
    - os: ubuntu-latest, target: x86_64-unknown-linux-gnu, platform: linux-x86_64
    - os: ubuntu-latest, target: aarch64-unknown-linux-gnu, platform: linux-aarch64
    - os: macos-latest, target: x86_64-apple-darwin, platform: macos-x86_64
    - os: macos-latest, target: aarch64-apple-darwin, platform: macos-aarch64
```

### GitLab CI/CD

The GitLab CI pipeline builds binaries sequentially or in parallel using four separate jobs:

```yaml
build:linux-x86_64:
  stage: build
  image: rust:1.93-slim
  variables:
    CARGO_TARGET: x86_64-unknown-linux-gnu
    PLATFORM: linux-x86_64

build:linux-aarch64:
  stage: build
  image: rust:1.93-slim
  variables:
    CARGO_TARGET: aarch64-unknown-linux-gnu
    PLATFORM: linux-aarch64

build:macos-x86_64:
  stage: build
  image: rust:1.93-slim
  variables:
    CARGO_TARGET: x86_64-apple-darwin
    PLATFORM: macos-x86_64

build:macos-aarch64:
  stage: build
  image: rust:1.93-slim
  variables:
    CARGO_TARGET: aarch64-apple-darwin
    PLATFORM: macos-aarch64
```

## 🚀 Release Workflow

### Creating a Release

```bash
# 1. Run the automated release script
./scripts/release.sh 0.2.0

# 2. Push changes to trigger CI/CD
git push origin main
git push origin 0.2.0
```

### What Happens

1. **Version Update**
   - `Cargo.toml` version is updated
   - `CHANGELOG.md` is updated with new release section

2. **Tag Creation**
   - Git tag created with version number (e.g., `0.2.0`)

3. **CI Pipeline Triggers**
   - GitHub Actions builds all 4 platform binaries
   - GitLab CI builds all 4 platform binaries

4. **Artifacts Generated**
   - Binaries placed in `dist/` directory
   - Named as `fishez-{platform}`

5. **Release Creation**
   - GitHub automatically uploads binaries as release assets
   - GitLab uploads binaries as release assets

### Release Pipeline Steps

#### GitHub Actions Release
```yaml
release:
  on:
    push:
      tags:
        - 'v*'
  steps:
    - name: Package binary
      run: |
        mkdir -p dist
        cp target/{platform}/release/fishez dist/fishez-{platform}

    - name: Upload binary to release
      uses: softprops/action-gh-release@v1
      with:
        files: dist/fishez-{platform}
```

#### GitLab CI Release
```yaml
release:
  stage: release
  rules:
    - if: '$CI_COMMIT_TAG'
  artifacts:
    paths:
      - release-artifacts/
  release:
    tag_name: '$CI_COMMIT_TAG'
    description: 'Release of fishez version $CI_COMMIT_TAG'
    assets:
      links:
        - name: 'fishez-linux-x86_64'
          url: 'https://gitlab.com/ioma8/fishez/-/jobs/$CI_JOB_ID/artifacts/file/dist/fishez-linux-x86_64'
          link_type: 'other'
        - name: 'fishez-linux-aarch64'
          url: 'https://gitlab.com/ioma8/fishez/-/jobs/$CI_JOB_ID/artifacts/file/dist/fishez-linux-aarch64'
          link_type: 'other'
        - name: 'fishez-macos-x86_64'
          url: 'https://gitlab.com/ioma8/fishez/-/jobs/$CI_JOB_ID/artifacts/file/dist/fishez-macos-x86_64'
          link_type: 'other'
        - name: 'fishez-macos-aarch64'
          url: 'https://gitlab.com/ioma8/fishez/-/jobs/$CI_JOB_ID/artifacts/file/dist/fishez-macos-aarch64'
          link_type: 'other'
```

## 📁 Artifact Structure

### CI Build Artifacts
```
dist/
├── fishez-linux-x86_64
├── fishez-linux-aarch64
├── fishez-macos-x86_64
└── fishez-macos-aarch64
```

### Release Assets
- **GitHub**: Automatically uploaded as release attachments
- **GitLab**: Uploaded as release assets with direct download links

## 🔧 Building Locally

### Single Platform Build
```bash
# Linux x86_64
cargo build --release --target x86_64-unknown-linux-gnu

# Linux ARM64
cargo build --release --target aarch64-unknown-linux-gnu

# macOS x86_64
cargo build --release --target x86_64-apple-darwin

# macOS ARM64
cargo build --release --target aarch64-apple-darwin
```

### Multi-Platform Build
```bash
# Using the build script
./scripts/build-multiplatform.sh 0.2.0

# Manual multi-platform build
for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin; do
    rustup target add $target
    cargo build --release --target $target
done
```

## 🧪 Testing

### Unit Tests
```bash
cargo test --lib
```

### Integration Tests
```bash
cargo test --test integration_flows
```

### Cross-Platform Testing
```bash
# After building for a target, test it
./target/{target}/release/fishez --help

# Or install and test
cargo install --path .
./fishez --help
```

## 📊 Pipeline Status

### GitHub Actions CI
- **Build Job**: Runs in parallel for all 4 platforms
- **Test Job**: Runs after build, downloads all artifacts
- **Artifacts**: Available for 7 days

### GitLab CI
- **Build Jobs**: 4 sequential jobs for each platform
- **Test Job**: Runs after all builds
- **Artifacts**: Available for 30 days
- **Release**: Automatically creates release when tag is pushed

## 🚦 Pipeline Stages

### GitHub Actions
1. **Build** - Multi-platform compilation
2. **Test** - Quality checks
3. **Docs** - Documentation generation
4. **Security** - Vulnerability scanning

### GitLab CI
1. **Build** - Platform-specific builds
2. **Test** - Unit and integration tests
3. **Quality** - Clippy, fmt, tree check
4. **Docs** - Documentation generation
5. **Security** - Vulnerability scanning
6. **Release** - Create release with binaries

## 🎯 Best Practices

1. **Versioning**: Always increment version numbers correctly
2. **Testing**: Test binaries locally after build
3. **CI**: Always review pipeline logs for any failures
4. **Changelog**: Document all changes before releasing
5. **Git Tags**: Use semantic versioning for tags (e.g., `v0.2.0`)

## 🐛 Troubleshooting

### Build Failures
- Check Rust toolchain compatibility
- Verify target triple is installed
- Review CI logs for specific error messages

### Release Failures
- Ensure tag format is `v{version}` (e.g., `v0.2.0`)
- Verify all build jobs passed
- Check platform-specific runtime dependencies

### Platform Issues
- **macOS**: Ensure Code Signing for distribution
- **Linux**: Verify library dependencies are static or available
- **ARM64**: Test on actual ARM64 hardware when possible

## 🔗 Resources

- [Rust Cross-Compilation](https://rust-embedded.github.io/book/intro/platform-support.html)
- [GitHub Actions Multi-Platform Build](https://github.com/actions/runner-images)
- [GitLab CI Documentation](https://docs.gitlab.com/ee/ci/)
- [Semantic Versioning](https://semver.org/)

## 📝 Example Release Flow

```bash
# 1. Prepare for release
./scripts/release.sh 0.2.0

# 2. Review changes
git diff
git log --oneline -3

# 3. Push to trigger CI
git push origin main
git push origin 0.2.0

# 4. Monitor pipelines
# GitHub: https://github.com/ioma8/fishez/actions
# GitLab: https://gitlab.com/ioma8/fishez/-/pipelines

# 5. Verify release
# GitHub: https://github.com/ioma8/fishez/releases
# GitLab: https://gitlab.com/ioma8/fishez/-/releases

# 6. Download and test binaries
wget https://github.com/ioma8/fishez/releases/download/v0.2.0/fishez-linux-x86_64
chmod +x fishez-linux-x86_64
./fishez-linux-x86_64 --help
```

## 📞 Support

If you encounter issues with multi-platform builds or releases:
1. Check the CI pipeline logs
2. Review this guide
3. Open an issue on GitHub
4. Check existing issues for similar problems
