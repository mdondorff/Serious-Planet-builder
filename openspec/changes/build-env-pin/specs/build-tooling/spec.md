## ADDED Requirements

### Requirement: BUILD-050 The build environment is pinned
`rust-toolchain.toml` SHALL name one exact Rust release (X.Y.Z, never a channel name), and every `dtolnay/rust-toolchain` step of `.github/workflows/ci.yml` SHALL install that release. `Cargo.lock` SHALL be committed and not ignored. No build file (`.cargo/config.toml`, a `Cargo.toml`, the CI workflow) SHALL set a target CPU, a target feature, a fast-math style flag or `RUSTFLAGS`; the build profile stays as `Cargo.toml` defines it.

Verify: A
Status: active
Source: Architect 2026-11-11 (ADR 0114 point 6; external review condition 2), owner 2026-11-12 (audit lists: every approved list a queue row), BUILD-005

#### Scenario: The pin
- **WHEN** the repository files are read
- **THEN** the toolchain is one exact release, every CI toolchain step names it, `Cargo.lock` is committed, and no file sets a target CPU, feature or fast-math flag; a moving `stable` in CI, a missing `Cargo.lock` or a `target-cpu` line fails the test naming the file and the line
