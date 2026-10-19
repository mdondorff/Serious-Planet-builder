## Why
`cargo xtask prepush` on a docs-only diff (only `spikes/**/results/**`, `docs/` or `openspec/`) selected all 12 crates for `test fast` and took 465 s (opus-planner, 2026-10-19; the log of the spikes branch). `packages_for_changes` fell back to "everything" for any change without a crate path, a rule meant for "nothing changed". CLAUDE.md defines a docs-only change as one that needs only `spec-lint`.

## What Changes
- `packages_for_changes`: an empty list of changed paths is the whole workspace (BUILD-003); a Cargo manifest, `Cargo.lock`, `.cargo/`, `.config/`, `crates/testkit` or `crates/perf` is the whole workspace; a crate path selects that crate and those above it; `xtask/`, `.github/`, `deny.toml`, `openspec/changes/`, `tests/goldens/` and script files (`.sh`, `.ps1`, `.bat`, `.cmd`), which the repository check tests read, select `xtask`; `clippy.toml` selects `core` and `spikes/mouse-look/` (replayed traces) `frame`; any other path (docs, the rest of openspec, `.claude`, other spikes, root `*.md`, other tool configuration) selects no crate. The verifier of the first version listed the tier-A tests that read such files; each is covered.
- `test fast` with no crate selected says so and succeeds without building.
- `prepush` with no crate path in the diff runs `spec-lint` instead of `check` and `test fast`, and `test gpu` when a golden path changed (BUILD-016).
- BUILD-021 and tier-A tests (docs-only selects none; a crate path selects that crate; `Cargo.lock` and workspace files select everything; nothing changed selects everything).
- The test that asserted the old fallback (`docs/status.md` runs everything) follows the new rule.

## Classes and bounds (ADR 0035)
No field and no shader value is added; build tooling.

## Out of scope
`cargo xtask check` itself (fmt, clippy, layering) is unchanged; a docs-only diff skips it in `prepush` because nothing it reads changed.
