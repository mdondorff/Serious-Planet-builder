## Why

The walking skeleton needs one CPU-generated tile, and the generator-side metamorphic properties (border agreement across faces, partition invariance, thread independence, cross-OS hashes) must hold from the first generator, not be discovered at M3.

## What Changes

- `generators`: CPU reference height generator (3D value-noise fBm on the unit direction), `TileData` with a versioned content hash, `generate_region` with fixed output order across thread counts, CPU dumps (tile height map, unfolded six-face cube net) as PNG.
- Tests: committed known-answer tile hashes (run on Windows and Linux CI), thread-count independence, border agreement including across cube faces, partition invariance, coarse/fine agreement, net and height-map dump properties.
- GEN-001..006 and GEN-009 become `active`. GEN-007 (staged generation), GEN-008 (CPU/GPU differential) and the definition-read recording stay planned for M3.

## Capabilities

### Modified Capabilities
- `generator-pipeline`: GEN-001, 002, 003, 004, 005, 006, 009 active.

## Impact

`crates/generators` (new code, `png` and `libm` dependencies; no graphics crate). GEN-003 is verified locally on Windows only until CI runs on Linux.
