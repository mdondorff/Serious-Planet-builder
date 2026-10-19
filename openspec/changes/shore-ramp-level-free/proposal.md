## Why
SPIKE-16 (`spikes/water-motion-gap/results/results.md`) diagnosed the owner's "dark patches on the shallow sea while the camera moves" (2026-10-18): the water is drawn, the patches are terrain albedo. The seabed-to-shore ramp (REND-030) was the ramp of the drawn node's level, so one seabed point 2.5 m deep has luminance 0.374 under L17 and 0.076 under L19 and finer. A moving camera draws the cover (L19 to L20 near tiles) where a still camera draws the exact plan (L16 to L19), so the albedo steps along tile borders. A drawn level is not a quantity of the scene.

## What Changes
- The ramp at a fragment is `max(shore_blend_m, min(per_m * d, 8 m))`, `d` the fragment's distance from the camera and `per_m = SHORE_RAMP_K * tau_px * pixel_angle` (`LodParams::shore_ramp_per_m`), carried once per plan (`TerrainPlan::shore_ramp_per_m`, globals `extra.y`). It no longer depends on the node that draws the fragment.
- Removed: `LodParams::shore_ramp_min_m(level)`, `TerrainDraw::shore_ramp_min_m`, the node record's ramp word, the morph blend `shore_ramp_at(z, morph)`. New: `lod::shore_ramp_at(per_m, distance)` (CPU and WGSL).
- At the far end of a node's drawing range the new ramp equals the old one (`K s(level - 1)` up to the cap), so the safety against the shoreline flash measured with k = 2 holds; nearer to the camera the ramp is smaller than the old per-node constant, never smaller than the selection's tolerance at that distance.
- Plan text: the per-node `shore_ramp <h32>` token is gone (read and ignored); a plan-level line `shore_ramp_per_m <h32>` appears when the value is not zero. The plan snapshots in `crates/frame/tests/data` and the floor-0 plan hashes change by that (node counts equal), a declared change.

## Impact
- Plan text: `TERRAIN_PLAN_FORMAT_VERSION` stays 1 (the reader accepts old node tokens). A plan or repro bundle recorded before this change loads and ramps by the look's blend alone, but its recorded `plan_hash` no longer equals the hash of the re-serialised plan, so it does not replay hash-identical.
- API: `TerrainDraw::shore_ramp_min_m` and `LodParams::shore_ramp_min_m` are removed; `TerrainPlan::shore_ramp_per_m` and `lod::shore_ramp_at(per_m, distance)` are new.
- Spec: REND-030 (MODIFIED).
- Tests: tier A `the_ramp_is_a_function_of_the_fragments_distance_and_the_per_level_rule_it_replaces_gave_a_factor_of_five_between_adjacent_levels` (the old rule: 0.374 against 0.076 between L17 and L19), the ramp function and its equality with the old rule at the far end of each level, the plan text and the node record without a ramp (render unit test); tier B `oracle_lit_shading` (shader against reference); tier C `gpu_water` coastal-flight flicker (REND-030 scenario "No flash on the coastal flights", 0.5 % of shore-band pixels; ATMO-017) re-run on both adapters.
- Goldens: lit and water views where the seabed is within the ramp move; they go through `golden-review`, lit views escalate to the owner's `/bless`.
- numerics-reviewer: the shader (distance from camera-relative f32, CON-11).
