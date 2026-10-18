## Why
SPIKE-15: a one-line edit costs 89 s (frame) and 31 s (app) of build before the first test; linking with `link.exe` and full debug info in every test binary dominate.

## What Changes
- `x86_64-pc-windows-msvc` links with `rust-lld`.
- Dev profile: `debug = "line-tables-only"` for workspace crates, `debug = false` for dependencies.

## Impact
Spec: BUILD-011 added (build-tooling). Build configuration only; code generation, tests and goldens unchanged. Everyone rebuilds once. Release and perf builds link with `rust-lld` too (verified, `cargo build --release -p planet`).
