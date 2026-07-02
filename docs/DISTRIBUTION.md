# Distribution

## `jpgfromraw-lib`

`jpgfromraw-lib v0.7.0` is published from `ioma8/jpgfromrawlib`.
Publish it again only when the RAW preview library changes:

```bash
# in ioma8/jpgfromrawlib
cargo login
cargo publish --dry-run
cargo publish
```

Required fork metadata:

```toml
name = "jpgfromraw-lib"
repository = "https://github.com/ioma8/jpgfromrawlib"
license = "MIT"
description = "Library fork of jpgfromraw: embedded JPEG extraction from RAW files"
```

Keep upstream MIT attribution in `authors`.

## Release Runbook

```bash
./scripts/release.sh 0.3.0
git push && git push --tags
cargo publish
# update ioma8/homebrew-tap Formula/fishez.rb version + sha256s, then push
```

Run the script from a clean working tree. It creates the release commit and
annotated `vX.Y.Z` tag.

The tag push triggers `.github/workflows/release.yml`, which publishes these
assets for `install.sh`:

```text
fishez-macos-aarch64
fishez-macos-x86_64
fishez-linux-aarch64
fishez-linux-x86_64
```

## Homebrew Tap Formula

Copy this to `ioma8/homebrew-tap/Formula/fishez.rb` and replace the version and
sha256 values after each GitHub release:

```ruby
class Fishez < Formula
  desc "Lightning-fast terminal file manager for developers"
  homepage "https://github.com/ioma8/fishez"
  version "0.3.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/ioma8/fishez/releases/download/v#{version}/fishez-macos-aarch64"
      sha256 "<sha256>"
    end
    on_intel do
      url "https://github.com/ioma8/fishez/releases/download/v#{version}/fishez-macos-x86_64"
      sha256 "<sha256>"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ioma8/fishez/releases/download/v#{version}/fishez-linux-aarch64"
      sha256 "<sha256>"
    end
    on_intel do
      url "https://github.com/ioma8/fishez/releases/download/v#{version}/fishez-linux-x86_64"
      sha256 "<sha256>"
    end
  end

  def install
    bin.install Dir["fishez-*"].first => "fishez"
  end

  def caveats
    <<~EOS
      To enable the fz cd-on-exit wrapper, add this to your shell rc:
        eval "$(fishez --init)"
    EOS
  end

  test do
    assert_match "fz()", shell_output("#{bin}/fishez --init")
  end
end
```

Get asset shas with:

```bash
shasum -a 256 fishez-*
```

## Clean-Machine Verification

```bash
curl -sfL https://raw.githubusercontent.com/ioma8/fishez/main/install.sh | sh
fishez --init | grep 'fz()'
```

```bash
brew install ioma8/tap/fishez
fishez --init | grep 'fz()'
fishez
```

```bash
CARGO_HOME="$(mktemp -d)" cargo install fishez
fishez --init | grep 'fz()'
```

For Homebrew, verify on a machine without Rust installed. For Cargo, verify the
build no longer pulls a git dependency. Homebrew and Cargo should print or
document the `eval "$(fishez --init)"` setup step; only `install.sh` modifies
`.zshrc` / `.bashrc` automatically.
