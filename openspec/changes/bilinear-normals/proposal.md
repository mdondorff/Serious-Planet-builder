## Why
- The viewer mesh splits every cell along `a-c` (`planet_generators::mesh::grid_indices`), and the lit paths shade with the vertex normal interpolated over each triangle. A smooth crease across the split diagonal renders as a sawtooth of cell size; along it, it stays straight (spikes #184g; G3-9's candidate-neutral item; the lead's design note on the viewer mesh split, 2026-11-24, option c1: bilinear texel normals in the fragment stage; lead #200, 2026-11-24).
- Measured through the real compact renderer (TEST-153, WARP): a scarp at 135 degrees turned back by a quarter turn differs from the same scarp at 45 degrees by a mean of 1.94 and a p99 of 22.6 luminance steps (0-255), all of it along the scarp's edges in a per-cell pattern.

## What changes
- REND-068: the lit fragment stages of the compact renderer shade with the bilinear interpolation of the cell's 4 stored texel normals at the fragment's tile coordinates (`tile_normal`, normalised).
  - A previewed node keeps the vertex normal.
  - The debug stage (`fs_view`) keeps the vertex normal, so its pictures stay bit-identical. The lit-stage overlays (tiles, mesh, stale, region) tint the lit colour and change with it.
  - The reference renderer has no lit path (REND-031): its `tile_normal` returns the vertex normal only so that the shared shader tail compiles.
- No geometry, mesh, slot, key, pool or morph change: the triangulation and REND-009's morph targets are unchanged.
- TEST-153: the 45 against 135 degree scarp through the real renderer.
  - Lit stage: mean 1.94 / p99 22.6 before, 0.044 / 0.93 after.
  - Lit-with-atmosphere stage: mean 1.27 before, 0.435 after. Its p99 (15.3 before, 14.8 after) is a residual of centre-line spots that the normal does not cause, reported to the lead.
- A tier-A shader-text scan pins which stages read `tile_normal` and that the debug views keep `in.normal`.

## Expected golden changes:
- lit_aerial20km_ev15: REND-068, the lit stage's normal
- lit_ground_eye0p5m_natural_ev14p5: REND-068
- lit_ground_eye1p8m_natural_ev14p5: REND-068
- lit_ground_march_10deg_ev14p5: REND-068
- lit_ground_march_2deg_ev14p5: REND-068
- lit_ground_march_aerial_10deg_ev14p5: REND-068
- lit_ground_march_aerial_2deg_ev14p5: REND-068
- lit_orbit_ev15: REND-068
- lit_orbit_ev_bands: REND-068
- lit_orbit_lighting_ev15: REND-068
- sky_aerial20km_ev15: REND-068, the lit-with-atmosphere stage's normal
- sky_orbit_ev15: REND-068
- water_class_coast_aerial2km: REND-068, the lit ground around the water
- water_class_lake_mask_stripes: REND-068
- water_class_lava_lake: REND-068
- water_class_orbit: REND-068
- The REND-016 known-answer hashes of the lit atmosphere frame and the lit water frames move with these pictures (REND-068's declared change, not the texel's rounding).
- No debug-view golden moves (WARP, 2026-11-25: the 16 above are every candidate; the debug stage keeps the vertex normal).

## Bound tests:
- lit_orbit_ev15: gpu_quad_split::a_scarp_across_the_cell_diagonal_is_lit_as_one_along_it, terrain::tests::the_lit_stages_shade_with_the_bilinear_texel_normal_and_the_debug_views_keep_the_vertex_normal

## Impact
- Spec deltas: REND-068 (renderer), TEST-153 (testing).
- Code: `crates/render/src/compact.rs` (`tile_normal`), `reference.rs` (the stand-in), `terrain.rs` and `atmosphere.rs` (the two lit fragment stages); the test `crates/render/tests/gpu_quad_split.rs`.
- The look of B and RG alike: the lit goldens go to the owner's `/bless` through golden-review. The evidence sheet (before and after, 45 and 135 degrees, WARP) is in the lead's digest (SeriousPlanetData, ADR 0121); TEST-153 writes the same pictures to `target/review/quad-split/`. The PR waits for the lead's review.
