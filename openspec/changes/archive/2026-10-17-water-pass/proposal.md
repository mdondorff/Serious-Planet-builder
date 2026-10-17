## Why
M5 item 7 (ADR 0041 points 4 to 6, 8 and 9, ADR 0051 points 6 and 8, the owner's M5 constraints 1 to 5): the sea has a field W (item 6) but nothing draws it. This change draws it: water on the terrain's own grid, Gerstner waves with phases reduced on the CPU, the shoreline from `W - h_morph`, the surface lit in HDR with the atmosphere over it, the synthetic lava lake through the same pass (the retrofit's rendering half), and the coastal-flight flicker metric. It also decides the per-tile storage of W (ADR 0051 point 6: option 1, the per-tile CPU query, no storage; the planner's steer of 2026-10-17: no new per-format storage) and the coast for a sea level other than 0 (ADR 0053).

## What Changes
- `frame::look::water` (GPU-free, ATMO-013 to ATMO-015, ATMO-017): the f64 references of the wave set, the CPU phase reduction, the shoreline functions, the surface radiance and the flicker metric (reprojection with the f64 poses).
- `frame::water` (ATMO-012): `WaterPlan` on `TerrainPlan` (nodes, levels, liquid slots, reduced phases, the liquids' appearance), the per-tile query `WaterTileCache` over any `WaterSource` (`WaterField` implements it), text form.
- `render::water`, `render::water_gpu` (ATMO-014, ATMO-016): the water shaders (both tile formats, with and without the atmosphere's lighting), the copy of the scene target, the pass, the probes; the terrain vertex stages split into `terrain_vertex` plus a thin entry so the water stage calls them; the render graph gains `scene-copy` and `water`.
- Look file schema 3 to 4 (`[body.<n>.water]`, nine presentation parameters), identity migration, fixture, LOOK-001 and LOOK-002 modified.
- Viewer (VIEW-008, VIEW-009): the world computes its ocean with its bake, the window plans and draws water, and the tier-C test of the atmosphere-does-not-fit fallback.
- The frustum cut keeps the water of tiles whose seabed the view does not see (`HeightField::water_cull_level`, `generators::water::WaterBounded`, ATMO-016): found by the coastal flight (a band of unwatered seabed at the bottom of a low view).
- ADR 0053 (proposed): the readings of ADR 0041 points 4 to 6 built here, the coast for a nonzero sea level, the flicker metric's definition.

## Impact
Specs: ATMO-004 modified, ATMO-012 to ATMO-018 added; LOOK-001 and LOOK-002 modified; VIEW-008 and VIEW-009 added. No `MESH_VERSION` or `GLOBAL_VERSION` change; no key moves; the debug path and its goldens are unchanged (a plan without water draws what it drew). The look schema is 4.

Expected golden changes:
- water_class_coast_aerial2km: new first golden of class `water` (the class view of the coast at 2 km, discrete bands)
- water_class_coast_low3m: new first golden of class `water` (the class view of the coast at 3 m)
- water_class_orbit: new first golden of class `water` (the class view of the planet from orbit)
- water_class_lava_lake: new first golden of class `water` (the class view of the synthetic lava lake's basin)

ADR 0037 conditions:
- water_class_coast_aerial2km: debug-view=water_class_coast_aerial2km; bound-tests=gpu_water::the_class_view_follows_the_depth_of_the_terrain_under_the_water_in_both_tile_formats, water::the_shoreline_functions_are_zero_at_the_waterline_continuous_and_discrete_by_class, oracle_water::the_waves_match_the_reference_within_a_millimetre_plus_a_thousandth_of_the_amplitudes_at_every_altitude, oracle_water::the_surface_radiance_matches_the_reference_within_one_percent_including_the_glint_the_foam_and_a_lava_lake
- water_class_coast_low3m: debug-view=water_class_coast_low3m; bound-tests=gpu_water::the_class_view_follows_the_depth_of_the_terrain_under_the_water_in_both_tile_formats, water::the_shoreline_functions_are_zero_at_the_waterline_continuous_and_discrete_by_class, oracle_water::the_waves_match_the_reference_within_a_millimetre_plus_a_thousandth_of_the_amplitudes_at_every_altitude, oracle_water::the_surface_radiance_matches_the_reference_within_one_percent_including_the_glint_the_foam_and_a_lava_lake
- water_class_orbit: debug-view=water_class_orbit; bound-tests=gpu_water::the_class_view_follows_the_depth_of_the_terrain_under_the_water_in_both_tile_formats, water::the_shoreline_functions_are_zero_at_the_waterline_continuous_and_discrete_by_class, oracle_water::the_waves_match_the_reference_within_a_millimetre_plus_a_thousandth_of_the_amplitudes_at_every_altitude, oracle_water::the_surface_radiance_matches_the_reference_within_one_percent_including_the_glint_the_foam_and_a_lava_lake
- water_class_lava_lake: debug-view=water_class_lava_lake; bound-tests=gpu_water::a_lava_lake_with_its_own_level_and_emission_goes_through_the_same_water_pass_as_the_sea, gpu_water::the_class_view_follows_the_depth_of_the_terrain_under_the_water_in_both_tile_formats, water::the_shoreline_functions_are_zero_at_the_waterline_continuous_and_discrete_by_class, oracle_water::the_waves_match_the_reference_within_a_millimetre_plus_a_thousandth_of_the_amplitudes_at_every_altitude, oracle_water::the_surface_radiance_matches_the_reference_within_one_percent_including_the_glint_the_foam_and_a_lava_lake

The four goldens stay in `tests/goldens/pending.txt`: the planner's change 7b (ADR 0052, the edge tolerance for first M5 goldens) blesses them with its pass. The lit water views (`target/review/water-inspection/`) are inspection-only, not goldens.
