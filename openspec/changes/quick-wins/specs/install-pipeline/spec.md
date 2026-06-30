## ADDED Requirements

### Requirement: Install via one-line script

The project SHALL provide a `install.sh` script at the repository root that downloads a prebuilt fishez binary for the user's platform and installs it to `/usr/local/bin/fishez`.

### Requirement: CI releases all platform binaries

The release workflow SHALL upload prebuilt binaries for all 4 platform targets (linux-x86_64, linux-aarch64, macos-x86_64, macos-aarch64) to every GitHub Release.

### Requirement: Published to crates.io

The fishez crate SHALL be published to crates.io so that `cargo install fishez` works.

#### Scenario: User installs via curl pipe

- **WHEN** a user runs `curl -sfL https://ioma8.github.io/fishez/install.sh | sh`
- **THEN** the script detects their OS and architecture, downloads the matching binary from the latest GitHub Release, and installs it to `/usr/local/bin/fishez`

#### Scenario: Release contains all binaries

- **WHEN** a new GitHub Release is created
- **THEN** the CI workflow uploads `fishez-linux-x86_64`, `fishez-linux-aarch64`, `fishez-macos-x86_64`, and `fishez-macos-aarch64` as release artifacts

#### Scenario: Install via cargo

- **WHEN** a user runs `cargo install fishez`
- **THEN** the crate compiles and installs the `fishez` binary
