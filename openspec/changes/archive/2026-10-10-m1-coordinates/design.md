## Context

ADRs 0001 (frames), 0003 (topology and tile ids), 0010 (determinism) and 0011 (reference surface), all proposed. Source: background report §2, §3, §17.

## Goals / Non-Goals

**Goals:** the COORD-001..010 requirements with tests at tier A. **Non-Goals:** field generation (next change), rendering, ellipsoid surface, equal-area warp.

## Decisions

- **Bit-identical borders without a canonicalisation table.** Points on a shared face edge are evaluated as `normalize(n + u·U + v·V)` where every vector component is a sum of exact terms: the face normal contributes exactly ±1 in one axis, `U` and `V` contribute exactly `u` and `v` in the other two axes, the rest are exact zeros. The warp returns exactly ±1 at `|s| = 1` (floating-point `tan(π/4)` is 0.9999999999999999) and is odd, and lattice coordinates `-1 + 2i/n` are exact dyadic rationals for power-of-two `n`. Both faces therefore build the same component values on the same axes and `normalize` is a pure function of them. Verified exhaustively for levels 0..3 over all faces and edges (`shared_border_samples_are_bit_identical_from_both_sides`) and for the eight corners.
- **Neighbour lookup by stepping past the edge midpoint** by 1e-3 of the tile width and locating the containing tile; edge tile boundaries coincide across faces, so the result is the unique edge-adjacent tile. Symmetry is tested for every tile through level 4 (exactly one way back).
- **Face tie rule:** largest absolute component, ties X then Y then Z, non-negative selects the positive face.
- **Exactness limit:** `sample_direction` asserts `level + log2(res) <= 52`; `from_direction` indexes with an exact `floor(c·n/2) + n/2`.
- **Tile-local f32 limit:** measured, not assumed: offsets from the tile centre meet 1 mm down to level 9 and finer, which includes the finest level 28 required by COORD-010; coarser storage needs another format (recorded as a test).
- **Hash:** SplitMix64 finaliser; the generator matches the reference sequence for seed 0. `hash_u64s` and `hash_lattice2` are ours; known answers are committed. Changing any output requires a `HASH_VERSION` bump.
- **API naming:** `geo_to_planet_fixed` / `planet_fixed_to_geo`, `face_to_plane` / `plane_to_face` (clippy's `to_*`/`from_*` self convention).

## Risks / Trade-offs

- Cross-OS equality of `libm` results is by construction (pure Rust) but only CI can confirm it; known-answer tests will show a divergence.
- The tangent warp's area distortion (ratio about 1.41 per the report) is accepted until the M2 gate.

## Open Questions

None blocking. (The spec-guardian suggested a spike for COORD-007; the exhaustive test and the construction above make it unnecessary.)
