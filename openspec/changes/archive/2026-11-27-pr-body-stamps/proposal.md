## Why
- ADR 0130 point 6 (proposed, applied on the owner's word of 2026-11-27): `pr-body-check` enforces the review stamps and the R/P firewall. The combined sprint process plan's item 6 builds it.
- CLAUDE.md's reviewer rules (numerics for `core`, `generators` and shaders; spec-guardian for spec changes) were text only; the path lines make them mechanical.
- The lead's ruling (2026-11-27): the path lines apply from the merge, on PRs pr-gate judges as code PRs; `Sprint:` only while a sprint file is active; the record lines only on PRs that add or change a record.

## What changes
- BUILD-075 (new): the `Numerics`, `Spec-Guardian`, `Research-Review`, `Owner-Look` and `Sprint` lines, and the call of `plan --pr` (BUILD-074).
- TEST-162 (new): the tier-A tests.
- `pr-gate.yml` passes `PR_TITLE` and `HEAD_REF`.

Expected golden changes: none (an xtask check).

## Impact
- `xtask/src/prbody.rs`, `xtask/src/main.rs` (usage line), `.github/workflows/pr-gate.yml`, `README.md` (the command's row).
- Every code PR that changes `crates/core/`, `crates/generators/`, a shader or `openspec/` needs the reviewer's line from the merge on.
