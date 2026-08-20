# Publishing Fishez

How a new Fishez version gets out to users. One source of truth: bump the
version once, push a tag, and the pieces below ship each channel. There are
three distribution channels:

| Channel | Trigger | Automated? |
|---|---|---|
| GitHub Release (binaries) | pushing `vX.Y.Z` tag | yes — `.github/workflows/release.yml` |
| Homebrew tap | `scripts/update-tap.sh` after the release | script, not CI |
| crates.io | `cargo publish` | manual (not wired into CI) |

## Prerequisites

- `gh` CLI authenticated: `gh auth status`
- `git` push access via SSH (`gh` uses `git@github.com:` remotes)
- crates.io: `cargo login` once, or a `CARGO_REGISTRY_TOKEN` (see [crates.io](#cratesio))
- For local brew verification: Homebrew installed

## 1. Bump version and create the tag

```bash
./scripts/release.sh 0.5.0
```

From a **clean working tree**. It:

1. Bumps `version` in `Cargo.toml`
2. Updates `CHANGELOG.md` (moves `[Unreleased]` into a dated `[0.5.0]` section)
3. Refreshes `Cargo.lock`, commits `Release v0.5.0`, creates annotated tag `v0.5.0`

## 2. Push — this is the trigger for the GitHub release

```bash
git push origin main
git push origin v0.5.0
```

Pushing the `v*` tag triggers `.github/workflows/release.yml`. That workflow
builds `cargo build --release` on 4 runners and uploads these assets to the
GitHub Release:

```text
fishez-macos-aarch64
fishez-macos-x86_64
fishez-linux-aarch64
fishez-linux-x86_64
```

**Do not publish any other channel before this finishes** — the Homebrew and
crates.io steps need the tag, and the tap script verifies shas against the
release.

Verify the run:

```bash
gh run list --repo ioma8/fishez --workflow release.yml
gh release view v0.5.0 --repo ioma8/fishez --json assets
```

Check the run's `headSha` matches the tag commit — that is the proof the
binaries were built from the released code (local builds are not
bit-reproducible, so don't compare hashes to a local build).

## 3. Homebrew tap

The tap `ioma8/homebrew-tap` (`brew install ioma8/tap/fishez`) ships the
release binaries. Update it with the one-command script:

```bash
./scripts/update-tap.sh 0.5.0        # version arg optional, defaults to 0.4.0
```

The script clones the tap via `gh`, then:

1. Fetches the authoritative sha256 digests for all 4 assets from the
   **GitHub API** (`gh release view --json assets`) — never from curl on the
   download URLs, which raced CDN caching / in-flight uploads and once served
   stale v0.3.1 bytes (see Troubleshooting).
2. Bumps `version` and all four `sha256` lines in `Formula/fishez.rb`
   (platform order: mac-arm, mac-intel, linux-arm, linux-intel).
3. **Self-verifies** the formula shas equal the API digests before pushing —
   it refuses to push on mismatch.
4. Commits `Update fishez to 0.5.0` and pushes to `ioma8/homebrew-tap`.

Verify from the remote:

```bash
gh api repos/ioma8/homebrew-tap/contents/Formula/fishez.rb --jq '.content' \
  | base64 -d | grep -E 'version "|sha256'
```

### Local machines with a path-based tap

If the tap was installed from a local directory
(`brew tap ioma8/tap ~/projects/customs/homebrew-tap`), brew's clone at
`/opt/homebrew/Library/Taps/ioma8/homebrew-tap` has that local path as its
origin and goes stale. Refresh it:

```bash
cd /opt/homebrew/Library/Taps/ioma8/homebrew-tap && git pull
brew info ioma8/tap/fishez   # should show the new version
```

Other users who do a plain `brew tap ioma8/tap` get the GitHub remote and are
always current.

## 4. crates.io

Publishing to crates.io is **manual** — nothing in CI does it yet.

### One-time setup

```bash
cargo login   # paste token from https://crates.io/settings/tokens
```

Or export the token for CI use (see below).

### Publish

```bash
cargo publish --dry-run   # sanity check, run from a clean tree
cargo publish
```

The crate metadata in `Cargo.toml` (`description`, `license`, `repository`,
`readme`, `keywords`, `categories`, `exclude`) is already set up; the
`exclude` list keeps `demo.gif`, `docs/`, `scripts/`, `tests/` etc. out of the
published crate.

### Optional: automate it in CI

Add a publish job to `.github/workflows/release.yml` (after `build`):

```yaml
  publish:
    runs-on: ubuntu-24.04
    needs: build
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo publish --token ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

and add the `CARGO_REGISTRY_TOKEN` secret in repo Settings → Secrets and
variables → Actions. Until then, publish locally after each release.

## 5. Verify everything

| Channel | Command | Expect |
|---|---|---|
| GitHub | `gh release view v0.5.0` | 4 assets, run on tag commit |
| Homebrew | `brew install ioma8/tap/fishez` | installs 0.5.0 |
| Homebrew | `fishez --init \| grep 'fz()'` | prints `fz()` wrapper |
| crates.io | `cargo info fishez` | version 0.5.0 |
| crates.io | `CARGO_HOME="$(mktemp -d)" cargo install fishez` | builds from registry, no git deps |

Run the Homebrew check on a machine without Rust to prove the binary (not a
build) is what ships.

## Release checklist (new version, end to end)

```bash
# 1. version + tag (clean tree required)
./scripts/release.sh 0.5.0

# 2. trigger GitHub release
git push origin main
git push origin v0.5.0

# 3. wait for release.yml to finish, confirm assets + tag commit
gh run watch --repo ioma8/fishez
gh release view v0.5.0 --repo ioma8/fishez --json assets

# 4. homebrew tap
./scripts/update-tap.sh 0.5.0

# 5. crates.io (manual)
cargo publish

# 6. verify (section above), including a brew install smoke test
```

## Troubleshooting

- **Tag pushed but no release**: check `gh run list --workflow release.yml`;
  the workflow only fires on tags matching `v*`. Confirm the tag is on the
  remote: `git ls-remote --tags origin`.
- **Tap script: "formula shas do not match the release"**: do not force it —
  the release binaries changed or the API digests are what the formula must
  equal. Re-run after the release workflow finishes.
- **Stale shas in the formula**: historical issue — the old script fetched
  hashes with `curl` from download URLs while the release was still
  uploading, and served v0.3.1 bytes for v0.4.0. The current script uses the
  GitHub API only, so it cannot regress this way.
- **`brew info` shows the old version on this machine**: path-based tap is
  stale — `git pull` in `/opt/homebrew/Library/Taps/ioma8/homebrew-tap`
  (see above).
- **crates.io rejects the package**: run `cargo publish --dry-run` and read
  the errors; usually a missing `repository`/`license` field or a bad
  `exclude` — all already configured for fishez.

## Rollback

GitHub: delete the release (`gh release delete v0.5.0`) and the tag
(`git push origin :refs/tags/v0.5.0`). Homebrew: point the formula back at the
previous version's shas (re-run `update-tap.sh` with the old version — it
reads whatever the release API serves). crates.io: versions are immutable —
you can `cargo yank` a bad version instead.

## Related docs

- `docs/DISTRIBUTION.md` — `jpgfromraw-lib` publishing and clean-machine verification
- `docs/RELEASE_GUIDE.md` — legacy GitLab CI-era guide, superseded by this file
- `.github/workflows/release.yml` — the tag-triggered binary release
- `scripts/release.sh`, `scripts/update-tap.sh` — the two release scripts
