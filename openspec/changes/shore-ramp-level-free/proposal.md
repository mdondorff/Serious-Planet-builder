## Why
SPIKE-16 (`spikes/water-motion-gap/results/results.md`) diagnosed the owner's "dark patches on the shallow sea while the camera moves" (2026-10-18): the water is drawn, the patches are terrain albedo. The seabed-to-shore ramp (REND-030) was the ramp of the drawn node's level, so one seabed point 2.5 m deep has luminance 0.374 under L17 and 0.076 under L19 and finer. A moving camera draws the cover (L19 to L20 near tiles) where a still camera draws the exact plan (L16 to L19), so the albedo steps along tile borders. A drawn level is not a quantity of the scene.

## What Changes
- The ramp at a fragment is `max(shore_blend_m, min(per_m * d, 8 m))`, `d` the fragment's distance from the camera and `per_m = SHORE_RAMP_K * tau_px * pixel_angle` (`LodParams::shore_ramp_per_m`), carried once per plan (`TerrainPlan::shore_ramp_per_m`, globals `extra.y`). It no longer depends on the node that draws the fragment.
- Removed: `LodParams::shore_ramp_min_m(level)`, `TerrainDraw::shore_ramp_min_m`, the node record's ramp word, the morph blend `shore_ramp_at(z, morph)`. New: `lod::shore_ramp_at(per_m, distance)` (CPU and WGSL).
- At the far end of a node's drawing range the new ramp equals the old one (`K s(level - 1)` up to the cap), so the safety against the shoreline flash measured with k = 2 holds; nearer to the camera the ramp is smaller than the old per-node constant, never smaller than the selection's tolerance at that distance.
- Plan text: the per-node `shore_ramp <h32>` token is gone (read and ignored); a plan-level line `shore_ramp_per_m <h32>` appears when the value is not zero. The plan snapshots in `crates/frame/tests/data` and the floor-0 plan hashes change by that (node counts equal), a declared change.

## Impact
- Spec: REND-030 (MODIFIED).
- Tests: tier A `the_same_seabed_point_has_the_same_albedo_whichever_level_of_tile_draws_it` (fails under the per-level rule: 0.374 against 0.076, ratio 4.9), the ramp function and its continuity with the old rule at the far end of each level, the plan text; tier B `oracle_lit_shading` (shader against reference); tier C `gpu_water` flicker budget (TEST-027, 0.5 % of shore-band pixels) re-run on the coastal flights.
- Goldens: lit and water views where the seabed is within the ramp move; they go through `golden-review`, lit views escalate to the owner's `/bless`.
- numerics-reviewer: the shader (distance from camera-relative f32, CON-11).
