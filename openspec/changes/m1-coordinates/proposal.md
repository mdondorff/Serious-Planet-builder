## Why

M1 needs the numeric foundation every later milestone stands on: typed f64/f32 frames, the reference surface, geo conversions, cube-sphere addressing and deterministic hashing. Retrofitting any of these is the documented failure mode of large-world engines (background report §3, §17). All of it is pure, GPU-free `core` code and can be falsified at tier A.

## What Changes

- `core`: `Vec3`, typed positions (`PlanetFixed`, `TileLocal`, `CameraRelative`), `ReferenceSurface` + `Sphere`, cube faces with the tangent warp behind `FaceMapping`, 64-bit `TileId` with parent/children/containing tile/cross-face neighbours and lattice sample directions, SplitMix64-based integer hashing and RNG (hash version 1).
- Tests: geo round trip, contract suite for surfaces, bijection and partition properties, neighbour symmetry over all faces/edges/corners, bit-identical border samples, known-answer hashes, banned platform-maths scan.
- COORD-001..010 become `active`.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `coordinates`: requirements COORD-001 to COORD-010 move from planned to active; COORD-007 states the exact-evaluation mechanism.

## Impact

`crates/core` only (plus its tests). Depends on `libm`. Windows/Linux hash equality for these functions is covered by known-answer tests that run in CI on both systems (CI has not run yet; see docs/status.md).
