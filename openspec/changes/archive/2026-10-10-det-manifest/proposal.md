## Why

ADR 0010 review (owner asked): ADR 0010 and GEN-003 say CI compares tile hashes between Windows and Linux, but no job did; each OS only checked four committed height hashes, so a per-OS difference in mesh output or the geo maths would pass CI.

## What Changes

- `generators::manifest`: `determinism_manifest()` (height and mesh content hashes for 35 tiles: every face at level 0, the four corner tiles of every face at level 3, deep tiles up to level 28; 88 reference-surface round trips; version lines).
- `planet generate --manifest <file>` writes it.
- `cargo xtask det-diff <a> <b>` compares two manifests and names every differing entry, version lines first.
- CI: each OS uploads its manifest; a `determinism` job that needs both diffs them.
- Modified: GEN-003 (manifest content and scenarios).

## Impact

`crates/generators`, `crates/app`, `xtask`, `.github/workflows/ci.yml`. No output changes, no goldens.
