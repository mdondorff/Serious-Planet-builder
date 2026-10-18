## Why
ADR 0062 point 3: levels 0 to 2 are not drawn. The 3x3 base surface is wrong there (0.46 px at orbit; SPIKE-12 Q1) and the alternative, a 5x5 base, costs +10 to +48 % vertex-stage time on every tile. A render floor at level 3 changes no tile in nine of the ten standard views and adds 129 nodes at orbit (52 level-2 nodes become 181 level-3 nodes, 182 over the default world, +0.147 ms indicative). SPIKE-12 Q5 step 3.

## What Changes
- frame: `LodParams::min_level` (default 3, `--floor-level N`, repro bundle `min_level`): selection splits every node below the floor; floor leaves are roots: no drawn parent, morph 0, interval `[1e30, 1e30]`, exempt from the plan's empty-interval check (LOD-011, LOD-004).
- streaming: the residency set, eviction rule, start-up load and cover treat floor tiles as the roots (384 at floor 3); R stops at the floor (STRM-001, STRM-007, STRM-015, STRM-016); the pre-fill covers levels 3..=4 (1,920 tiles, was 2,046 of levels 0..=4) and the pack drops the vertex-form records (STRM-021); a cold orbit start has no coarse stand-in (the pre-fill covers it).
- app: `--floor-level`, the meaning of `--prefill-levels`; the managed-loop tests that assume level-0 roots take the floor as a parameter.
- tests: orbit-20000km plans (181 level-3 nodes), floor 0 reproduces the old plans (`terrain_plans_floor0.txt`), floor leaves do not morph, cold start (17.76 s for the 384 roots with the pack ignored, `status.md`); only the orbit goldens and one 1-step view change, by declaration, and go to the owner's `/bless` (ADR 0062).

## Impact
Spec: LOD-011 added; LOD-004, LOD-009, STRM-001, STRM-007, STRM-015, STRM-016, STRM-021, STRM-022, TEST-008, TEST-018, TEST-019, TEST-021, TEST-024, ATMO-020 modified. STRM-021's delta is written on top of `compact-slot-v2`'s (payload version 3): archive that change first, or merge the two deltas. Not in this change: slot layout, pool arithmetic, removing the vertex pool (its 256 manager slots for levels 0-2 become unused and go with `remove-vertex-render-path`), the cold start is synchronous: the 384 floor tiles load before frame 0 (STRM-001) and the readiness report (ADR 0045 point 5) shows progress in the window title; the effective floor is min(floor, max_level) and only an explicit `--floor-level` above `max_level` is refused, naming both; the roots load is exempt from the upload cap (a load step before frame 0, roots resident for the body's life); `generate --prefill-dir` takes `--floor-level`. SPIKE-12 wrote 2,040 tiles for levels 3..=4; 6 x (64 + 256) = 1,920 is used here. STRM-006 gets a one-clause cross-reference to STRM-001's exemption at archive, after `compact-slot-v2` (which also modifies STRM-006), to avoid a double delta.

Expected golden changes:
- lit_orbit_ev15: the orbit globe is drawn from the 181 level-3 tiles instead of 52 level-2 tiles: coastlines and the limb move by a few hundred pixels
- lit_orbit_ev_bands: same orbit tiles, exposure-band view
- lit_orbit_temperature_ev15: same orbit tiles, temperature view
- lit_orbit_precipitation_ev15: same orbit tiles, precipitation view
- lit_orbit_lighting_ev15: same orbit tiles, lighting view
- sky_orbit_ev15: same orbit tiles under the atmosphere
- water_class_orbit: same orbit tiles, water class view
- terrain_orbit_tile_id: the tile-id view of the orbit: level-3 tiles instead of level-2 tiles (tile ids, levels, geometry)
- terrain_cube_corner_normals: leaves at the floor near the horizon no longer morph to a parent: 17 pixels on WARP and 23 on the RTX, one channel step

Bound tests:
- lit_orbit_ev15: floor::levels_below_the_floor_are_never_selected_and_only_the_orbit_view_changes, floor::floor_leaves_do_not_morph_and_the_plan_builder_accepts_their_interval_and_only_theirs
- lit_orbit_ev_bands: floor::levels_below_the_floor_are_never_selected_and_only_the_orbit_view_changes, floor::floor_leaves_do_not_morph_and_the_plan_builder_accepts_their_interval_and_only_theirs
- lit_orbit_temperature_ev15: floor::levels_below_the_floor_are_never_selected_and_only_the_orbit_view_changes, floor::floor_leaves_do_not_morph_and_the_plan_builder_accepts_their_interval_and_only_theirs
- lit_orbit_precipitation_ev15: floor::levels_below_the_floor_are_never_selected_and_only_the_orbit_view_changes, floor::floor_leaves_do_not_morph_and_the_plan_builder_accepts_their_interval_and_only_theirs
- lit_orbit_lighting_ev15: floor::levels_below_the_floor_are_never_selected_and_only_the_orbit_view_changes, floor::floor_leaves_do_not_morph_and_the_plan_builder_accepts_their_interval_and_only_theirs
- sky_orbit_ev15: floor::levels_below_the_floor_are_never_selected_and_only_the_orbit_view_changes, floor::floor_leaves_do_not_morph_and_the_plan_builder_accepts_their_interval_and_only_theirs
- water_class_orbit: floor::levels_below_the_floor_are_never_selected_and_only_the_orbit_view_changes, floor::floor_leaves_do_not_morph_and_the_plan_builder_accepts_their_interval_and_only_theirs
- terrain_orbit_tile_id: floor::levels_below_the_floor_are_never_selected_and_only_the_orbit_view_changes, floor::floor_leaves_do_not_morph_and_the_plan_builder_accepts_their_interval_and_only_theirs, gpu_terrain::ten_scripted_views_have_no_holes_and_exact_draw_counts
- terrain_cube_corner_normals: floor::levels_below_the_floor_are_never_selected_and_only_the_orbit_view_changes, floor::floor_leaves_do_not_morph_and_the_plan_builder_accepts_their_interval_and_only_theirs

Measured against the drafts (learning of 2026-10-18): the plans of the nine other views are the floor-0 plans in tiles and mesh keys, equal above the floor, with no morph at the floor; high-2000km at the 128 x 96 snapshot view changes (73 to 71 nodes); the default world's orbit is 182 nodes; R and D lose their levels 0-2 ancestors in every view (pins re-pinned); the six roots of floor 0 are exempt from the upload cap too (two scheduler digests re-pinned, `scheduler_equivalence.rs`); the pre-fill below the floor writes nothing and the perf scenes refuse a `--prefill-levels` below the floor; the pack markers say `levels F-N`, and a log from before says `0-N` and reads as floor 0; `load_startup_roots` and `load_startup_roots_ms` are recorded in the streaming logs; the water pre-fill oracle runs on the 384 tiles of level 3 (60 s under the full suite).
