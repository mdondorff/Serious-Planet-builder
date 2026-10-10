## Why

The second M2 step turns node selection into pixels: camera-relative terrain with reversed-Z depth, geomorphing and skirts, the debug views the report requires (tile ID, level, face, morph, normals, depth, height), and the ten scripted views that check for cracks and precision from orbit to 1 m.

## What Changes

- `frame`: `Camera` (reversed-Z infinite-far projection in f64, rounded once), `Frustum`, `select_nodes_in_view`, `TerrainPlan` / `plan_terrain` (camera-relative origins, morph intervals, colours), `scripted_views`.
- `generators::mesh`: tile meshes with tile-local f32 positions, normals, parent-grid morph targets and skirts; `TileId::sample_direction_ext` (one-sample halo).
- `render`: `execute_terrain` (one indexed draw per node, `Depth32Float`, compare Greater, clear 0, dynamic-offset node uniforms), terrain WGSL, shader scan for planet-scale values.
- `app`: `test-render --scene terrain --script N --view V`.
- `testkit`: `tests/goldens/pending.txt` lets named goldens wait for the owner's `/bless` without failing CI for a blessed adapter.
- Active: LOD-005, LOD-007 (new), REND-003, REND-004 (verified by f32 shader-path emulation), REND-005, REND-006 (views extended), REND-009 (new).

## Capabilities

### Modified Capabilities
- `terrain-lod`: LOD-005 (reworded to a measurable hole criterion); new LOD-007.
- `renderer`: REND-003, 004, 005, 006; new REND-009.
- `testing`: TEST-002 (pending list).

## Impact

`crates/frame`, `crates/generators`, `crates/render`, `crates/app`, `crates/testkit`, `crates/core` (halo sampling). Six new candidate goldens are listed in `tests/goldens/pending.txt`.
