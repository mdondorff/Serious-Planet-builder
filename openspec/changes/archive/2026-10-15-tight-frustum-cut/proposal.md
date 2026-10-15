## Why

M4 item 7c. The draw set's frustum test pads every node by the global `max_height_m` (9 km or more), so near the ground it removes almost nothing: at ground-1m 8,687 nodes are drawn, of which only about 1,100 own a visible pixel (SPIKE-05's CPU depth pass; most of the rest is occluded terrain, which stays out of scope, CON-03). LOD-008 already gives every tile proven height bounds and a bounding volume; the frustum test can use it.

## What Changes

- frame: after the cheap padded-sphere test (kept as a first filter, so far tiles still cost no height bounds), `select_impl` tests the frustum against the tile's own height-aware bounding volume (LOD-008), enlarged by the skirt depth because the skirts are drawn. A pure frustum test, conservative by construction (the volume covers the tile's horizontal extent and height range); no occlusion culling.
- terrain-lod spec: LOD-007 says the frustum pad is the tile's own height bound; a scenario for it.
- tests: the SPIKE-05 CPU depth pass as a test helper (`tests/support/depth_oracle.rs`); tier B `oracle_frustum_cut.rs` (no tile with a visible triangle is cut, ten standard views); tier A `frustum_cut.rs` (a cut tile has no vertex inside the frustum, reference and steep fields; ground-1m node count). TEST-011 per-view pins re-pinned as a declared change (below).

## Declared change (TEST-011)

Per-view node counts at cells 32, tau 6, 1440p, reference field, before to after: orbit 52 to 52, high-2000km 424 to 428 (the coarse tiles' skirts, up to tens of kilometres deep, now count in the first test; the old 9 km pad did not cover them), aerial-200km 645 to 630, aerial-20km 1071 to 892, low-2km 2561 to 1279, ground-200m 4425 to 1694, ground-1m 8687 to 2897, north-pole-1km 2612 to 1180, cube-corner-1km 3573 to 1606, face-edge-1km 3546 to 1574. Triangles, resident tiles and upload bytes follow (2,304 per node, one tile per node). The plan also gets cheaper (ad hoc, reference field: cold ground-1m 39.7 to 15.3 ms, warm 6.0 to 2.2 ms), because fewer nodes are visited and drawn.

Not in this change: occlusion culling (owner's decision, CON-03), the eroded-field counts of 7d.

Expected golden changes:
- terrain_low2km_morph: the tight cut leaves fewer leaves per level, and the morph start is equalised over the leaves of a level (LOD-008), so the morph differs by rounding in 2 pixels (max channel delta 1) on WARP; no other golden moves.
