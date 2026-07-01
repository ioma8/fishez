# Release Guide

This guide explains how to properly release new versions of Fishez using GitLab CI/CD.

## Quick Release Process

### 1. Run the automated release script
```bash
./scripts/release.sh 0.2.0
```

This will:
- Update `Cargo.toml` version
- Update `CHANGELOG.md`
- Create a git tag

### 2. Push changes to trigger CI/CD
```bash
git push origin main
git push origin 0.2.0
```

### 3. Monitor GitLab CI
- Go to: `https://gitlab.com/ioma8/fishez/-/pipelines`
- Watch the release pipeline run automatically
- The pipeline will build and create the release

### 4. Verify release
- Go to: `https://gitlab.com/ioma8/fishez/-/releases`
- Check that the release was created successfully
- Download the binary if needed

## Manual Release Process

### Step 1: Update Version in Cargo.toml
```toml
version = "0.2.0"  # Change from "0.1.0" to new version
```

### Step 2: Update CHANGELOG.md
```markdown
## [Unreleased]

### Added
- New features for this version

## [0.2.0] - 2024-02-16

### Added
- Feature 1
- Feature 2

[Unreleased]: https://github.com/ioma8/fishez/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/ioma8/fishez/releases/tag/v0.2.0
```

### Step 3: Create and Push Tag
```bash
git tag -a 0.2.0 -m "Release fishez 0.2.0"
git push origin 0.2.0
```

### Step 4: Update README.md (Optional)
If needed, update version numbers in the README.

## Understanding the CI/CD Pipeline

### Pipeline Stages
1. **Test**: Unit tests and integration tests
2. **Quality**: Clippy, fmt, tree check, binary size
3. **Build**: Release binary compilation
4. **Docs**: HTML documentation generation
5. **Security**: Vulnerability scanning
6. **Release**: Automated release creation

### What Gets Created
- **Binary**: `target/release/fishez` (uploaded as release asset)
- **Documentation**: `target/doc/` (uploaded as release asset)
- **GitLab Release**: Created automatically with tag

### Release Artifacts
- **Binary**: Executable for your OS
- **Documentation**: HTML docs with `index.html`

## After Release

### 1. Update README.md
Update version references and any feature lists if changed.

### 2. Commit and Push
```bash
git add .
git commit -m "Prepare release v0.2.0"
git push origin main
```

### 3. Verify Release on GitHub/GitLab
- Check that all tests pass
- Verify the release page looks correct
- Test downloading the binary if needed

### 4. Update Unreleased Section
```bash
# After release, update CHANGELOG.md:
sed -i.bak 's/\[Unreleased\]/[0.2.0] - $(date +%Y-%m-%d)/' CHANGELOG.md
rm CHANGELOG.md.bak
```

## Troubleshooting

### Tag not triggering release
- Check that the tag name matches the version exactly (e.g., `0.2.0` not `v0.2.0`)
- Verify the tag was pushed: `git ls-remote --tags origin`
- Check GitLab CI logs for errors

### Build failures
- Run local: `make release-ci`
- Check for compilation errors
- Review CI logs for specific failures

### Release not created
- Verify all pipeline stages pass
- Check that the release stage has a tag ref
- Ensure release assets are being uploaded

## Best Practices

1. **Semantic Versioning**: Follow MAJOR.MINOR.PATCH format
2. **Test Thoroughly**: Run tests before releasing
3. **Update Changelog**: Document all changes
4. **Tag Immediately**: Create tag right after version change
5. **Push Both**: Push both main branch and tag
6. **Review CI**: Check all pipeline stages pass
7. **Binary Test**: Verify binary works on your system

## Rollback Procedure

If a release needs to be reverted:

```bash
# Delete the release from GitLab (via web interface)
# Delete the tag
git tag -d 0.2.0
git push origin :refs/tags/0.2.0

# Reset version in Cargo.toml
# Revert CHANGELOG.md
```

## Creating a Pre-release

For testing before official release:

```bash
# Use pre-release version suffix (e.g., 0.2.0-alpha.1)
./scripts/release.sh 0.2.0-alpha.1

# Push tag
git push origin 0.2.0-alpha.1
```

## Checking Pipeline Status

```bash
# Check pipeline status
curl -s "https://gitlab.com/api/v4/projects/ioma8%2Ffishez/pipelines?ref=main" \
  -H "PRIVATE-TOKEN: <your_token>" | jq

# Or visit: https://gitlab.com/ioma8/fishez/-/pipelines
```

## References

- [GitLab CI/CD Documentation](https://docs.gitlab.com/ee/ci/)
- [Semantic Versioning](https://semver.org/)
- [Rust Release Process](https://doc.rust-lang.org/cargo/guide/releasing.html)
