## ADDED Requirements

### Requirement: BUILD-011 Fast edit-to-test build
On Windows the workspace SHALL link with `rust-lld` (`.cargo/config.toml`, target `x86_64-pc-windows-msvc`), and the dev profile SHALL build workspace crates with `debug = "line-tables-only"` and dependencies with `debug = false`, so backtraces keep file and line while local variables need a full-debug profile.

Verify: A
Status: active
Source: SPIKE-15

#### Scenario: Configuration present
- **WHEN** the repository check test reads `.cargo/config.toml` and `Cargo.toml`
- **THEN** the linker, `debug = "line-tables-only"` in `[profile.dev]` and `debug = false` in `[profile.dev.package."*"]` are all set

#### Scenario: Edit-to-test build time
- **WHEN** one line of a function body is edited and `cargo nextest run --no-run` builds the edited crate and every crate above it (`CARGO_BUILD_JOBS=10`, Rasierklinge, quiet, median of 3)
- **THEN** the frame crate edit builds in about 49 s (baseline 89 s) and the app crate edit in about 15 s (baseline 31 s)
