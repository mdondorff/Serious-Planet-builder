## Why

The parallel ADR review (ADR 0003) found, by hand, that mesh normals at cube-face borders disagree between the two tiles that share a vertex, and that the partition test was defined through the function it tests. A probe measured the mismatch first: **0.076 degrees** across a cube edge (0.0 within a face), larger than the 0.02 degrees seen earlier.

## What Changes

- Mesh normals come from `surface_normal`: central differences in a tangent frame that depends only on the vertex direction (and a level-dependent step), so shared vertices get bit-identical normals; the halo sampling is no longer needed.
- `TileId::corner_neighbors` (and `Corner`, `corner_direction`): three tiles at an ordinary corner, two at a cube corner; tested against a bit-identical-corner ground truth, with symmetry.
- The partition test uses an independent definition of tile membership (largest component, plain tangent formula, dyadic bounds).
- Tests: border normals over every tile of level 2 and every cube edge (96 cross-face pairs) below 1e-6 degrees.
- COORD-007 and REND-009 are amended. On the software adapter the (unblessed) terrain candidates are byte-identical to before: the smooth terrain hides a 0.08 degree difference in an 8-bit image.

## Capabilities

### Modified Capabilities
- `coordinates`: COORD-007. `renderer`: REND-009.

## Impact

`crates/core` (tile.rs), `crates/generators` (mesh.rs). Mesh generation does about four extra height evaluations per vertex.
