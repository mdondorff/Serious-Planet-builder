## ADDED Requirements

### Requirement: BUILD-021 A change with no crate path selects no crate
`cargo xtask test fast` SHALL select the tier-A tests of the crates the changed files name and every crate above them (BUILD-003), of `xtask` for a file under `xtask/` or `.github/`, and of every crate when the list of changed files is empty (nothing changed) or holds a Cargo manifest, `Cargo.lock`, a file under `.cargo/` or `.config/`, or a file of `crates/testkit` or `crates/perf`. A list of changed files none of which is any of these (for example only files under `docs/`, `openspec/`, `.claude/`, `spikes/`, `tests/goldens/`, root `*.md` files or tool configuration) SHALL select no crate: `test fast` SHALL say so and succeed without building, and `cargo xtask prepush` SHALL run `spec-lint` instead of `check` and `test fast` (and `test gpu` when the diff holds a golden-producing path, BUILD-016).

Verify: A
Status: active
Source: opus-planner (2026-10-19), BUILD-003, BUILD-016, CLAUDE.md (docs-only changes)

#### Scenario: Docs only
- **WHEN** the changed files are `docs/status.md` and `openspec/specs/viewer/spec.md`
- **THEN** no crate is selected, `test fast` runs nothing, and `prepush` runs only `spec-lint`

#### Scenario: A crate path, the workspace, nothing
- **WHEN** the changed files are `docs/a.md` and `crates/streaming/src/lib.rs`, or `docs/a.md` and `Cargo.lock`, or none
- **THEN** the first selects `planet-streaming` and the crates above it, the second and the third every crate
