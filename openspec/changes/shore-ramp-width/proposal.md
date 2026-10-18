## Why
The tracked "terrain shading at the beach flashes": the terrain's own shading at the shore flashed by more than a quarter between frames at 6.3 % (glide) and 36.8 % (descent) of the shore-band pixels. Attributed to one term: the 1 m seabed-to-shore albedo ramp, narrower than the mesh's height tolerance (REND-030).

## What changes
- Each plan node carries `shore_ramp_min_m` = `min(SHORE_RAMP_K * sample_spacing(level - 1), 8 m)`; the lit shader and the f64 reference use `max(shore_blend_m, shore_ramp_at(shore_ramp_min_m, morph))` as the ramp, where `morph` is the vertex's interpolated morph factor, so the ramp is continuous across level borders. Near the camera nothing changes (1 m).
- The plan text gains `shore_ramp <h32>` on each node line (old text still parses, as 0); the plan snapshots are regenerated.
- The coastal flights assert the terrain's own flash at most 0.5 % (0.000 % on both).

Expected golden changes:
- lit_orbit_ev15: coastline pixels of the coarse nodes, 4 pixels on WARP
- lit_orbit_ev_bands: coastline pixels of the coarse nodes, 3 pixels on WARP
- lit_aerial20km_ev15: coastline pixels of the 20 km view, 91 pixels on WARP
- lit_aerial20km_albedo_ev15: coastline pixels of the 20 km view, 90 pixels on WARP
- sky_orbit_ev15: coastline pixels of the coarse nodes, 3 pixels on WARP
- sky_aerial20km_ev15: coastline pixels of the 20 km view, 90 pixels on WARP
- water_class_orbit: coastline pixels of the coarse nodes, 4 pixels on WARP

Bound tests:
- lit_orbit_ev15: oracle_lit_shading::the_lit_radiance_matches_the_f64_reference_within_its_declared_bound, gpu_water::a_coastal_flight_at_low_altitude_flickers_within_the_budget_while_the_lod_changes_under_it
- lit_orbit_ev_bands: oracle_lit_shading::the_lit_radiance_matches_the_f64_reference_within_its_declared_bound
- lit_aerial20km_ev15: oracle_lit_shading::the_lit_radiance_matches_the_f64_reference_within_its_declared_bound, gpu_water::a_coastal_flight_at_low_altitude_flickers_within_the_budget_while_the_lod_changes_under_it
- lit_aerial20km_albedo_ev15: oracle_lit_shading::the_lit_radiance_matches_the_f64_reference_within_its_declared_bound
- sky_orbit_ev15: oracle_lit_shading::the_lit_radiance_matches_the_f64_reference_within_its_declared_bound
- sky_aerial20km_ev15: oracle_lit_shading::the_lit_radiance_matches_the_f64_reference_within_its_declared_bound
- water_class_orbit: gpu_water::a_coastal_flight_at_low_altitude_flickers_within_the_budget_while_the_lod_changes_under_it

Owner decision folded in (M5 gate 2, 2026-10-18): `water.shore_filter` defaults to 0 (the cheaper shore; `WaterLook::EARTH`, the look spec table, ADR 0053 status line); the other setting of the coastal flights and of the capture is now 1 (`@filter1`, `-filter1`). The seven goldens above are the same ones and move by the same pixels with the new default; the known-answer hashes of `gpu_water` are re-recorded (WARP local and RTX).
