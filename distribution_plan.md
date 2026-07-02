# Distribution Plan — one-command install everywhere

Goal: `brew install ioma8/tap/fishez`, `curl … | sh`, and `cargo install fishez` all
work. Order matters — each step unblocks the next.

## 1. Publish the raw-preview dependency to crates.io

`Cargo.toml` depends on `jpgfromrawlib = { package = "jpgfromraw", git = … }` — a fork
of cdown/jpgfromraw with a library API (`process_file_bytes`, `SUPPORTED_EXTENSIONS`,
`FindJpegType`). Git deps block `cargo publish`, and the name `jpgfromraw` on crates.io
belongs to upstream (cdown), so the fork must publish under its own name.

In the ioma8/jpgfromrawlib repo:
- `Cargo.toml`: `name = "jpgfromraw-lib"`, `repository = "https://github.com/ioma8/jpgfromrawlib"`,
  keep `license = "MIT"` and the original author in `authors` (attribution), add
  `description = "Library fork of jpgfromraw: embedded JPEG extraction from RAW files"`.
- `cargo login <token>` (crates.io → Account → API Tokens), `cargo publish --dry-run`,
  then `cargo publish`.

In fishez `Cargo.toml` — code needs zero changes thanks to the rename alias:
```toml
jpgfromrawlib = { package = "jpgfromraw-lib", version = "0.7" }
```

Fallback if publishing the fork is undesirable: vendor the ~3 used APIs into
`src/infrastructure/` (raw_image.rs is the only consumer, 94 lines). Prefer publishing.

## 2. Make fishez itself publishable

`Cargo.toml` `[package]` is missing all registry metadata. Add:

```toml
description = "Lightning-fast terminal file manager for developers: type-to-filter, quick view, fd/ripgrep, cd-on-exit"
license = "MIT"
repository = "https://github.com/ioma8/fishez"
readme = "README.md"
keywords = ["file-manager", "tui", "terminal", "crossterm"]
categories = ["command-line-utilities", "filesystem"]
exclude = ["demo.gif", "demo.tape", "docs/", "openspec/", "scripts/", "tests/", "*.md", "!README.md"]
```

Careful: `Fishez_logo.svg` is `include_bytes!`-ed by the renderer — it must NOT be
excluded. Verify with `cargo package --list`. Then `cargo publish --dry-run`, publish.

Result: `cargo install fishez` works → restore that line in README Quick Start.

## 3. GitHub Actions release workflow

New `.github/workflows/release.yml`, triggered by `push: tags: ["v*"]`. Asset names
must match what `install.sh` already downloads: `fishez-{macos,linux}-{x86_64,aarch64}`.

```yaml
name: Release
on:
  push:
    tags: ["v*"]
permissions:
  contents: write
jobs:
  build:
    strategy:
      matrix:
        include:
          - { os: macos-14,        asset: fishez-macos-aarch64 }
          - { os: macos-13,        asset: fishez-macos-x86_64 }
          - { os: ubuntu-24.04,    asset: fishez-linux-x86_64 }
          - { os: ubuntu-24.04-arm, asset: fishez-linux-aarch64 }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release
      - run: cp /tmp/fishez_target/release/fishez ${{ matrix.asset }}   # target-dir set in .cargo/config.toml
      - uses: softprops/action-gh-release@v2
        with:
          files: ${{ matrix.asset }}
          generate_release_notes: true
```

Linux runners need `sudo apt-get install -y libx11-dev` etc. only if the clipboard/
trash crates require it — mirror whatever `ci.yml` already installs. This makes the
existing `install.sh` one-liner (`releases/latest/download/…`) fully functional;
remove its "needs a published GitHub release" header caveat afterwards.

## 4. Homebrew tap

New repo `ioma8/homebrew-tap`, file `Formula/fishez.rb`:

```ruby
class Fishez < Formula
  desc "Lightning-fast terminal file manager for developers"
  homepage "https://github.com/ioma8/fishez"
  version "0.3.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/ioma8/fishez/releases/download/v#{version}/fishez-macos-aarch64"
      sha256 "<sha256 of asset>"
    end
    on_intel do
      url "https://github.com/ioma8/fishez/releases/download/v#{version}/fishez-macos-x86_64"
      sha256 "<sha256>"
    end
  end
  on_linux do
    url "https://github.com/ioma8/fishez/releases/download/v#{version}/fishez-linux-x86_64"
    sha256 "<sha256>"
  end

  def install
    bin.install Dir["fishez-*"].first => "fishez"
  end

  test do
    assert_match "fz()", shell_output("#{bin}/fishez --init")
  end
end
```

`shasum -a 256 <asset>` after the release. Users: `brew install ioma8/tap/fishez`.
(Automating the sha bump per release is a later nicety, not part of this plan.)

## 5. README after first release

Quick Start becomes, in order:
```bash
brew install ioma8/tap/fishez                    # macOS / Linuxbrew
curl -sfL https://raw.githubusercontent.com/ioma8/fishez/main/install.sh | sh   # any Unix
cargo install fishez                             # from source
```
Keep the `eval "$(fishez --init)"` + `fz` lines unchanged.

## 6. Release runbook (each release)

1. `./scripts/release.sh 0.3.0` — bumps Cargo.toml + CHANGELOG (review its commit/tag
   behavior once; ensure it tags `v0.3.0`).
2. `git push && git push --tags` → workflow builds and publishes the GitHub release.
3. `cargo publish` (fishez; the dep only when it changed).
4. Update tap formula version + sha256s, push.

## 7. Verification

- Clean container/VM: run the curl one-liner → `fishez --init` prints `fz()`.
- `brew install ioma8/tap/fishez` on a Mac without Rust → app launches, F1 footer
  shows the right version.
- `cargo install fishez` in a fresh CARGO_HOME → builds without the git dep.

## Out of scope

Windows binaries/scoop, crates.io publish automation via CI (trusted publishing),
brew-core submission (needs notability first — this plan is how it gets there).
