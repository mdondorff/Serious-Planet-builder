## Why
M5 item 5 part 2 (ADR 0040, ADR 0050 proposed): the GPU side of the atmosphere on top of the reference of `atmosphere-reference`: the render-graph passes, the composite over the terrain, the terrain's atmosphere lighting, the tier-B oracle, the temporal-stability scenario at 1 m (ADR 0043) and the first sky goldens.

## What Changes
- **Render graph (ATMO-011):** `LitShape::atmosphere`; passes `atmosphere-luts`, `sky-view`, `aerial-perspective`, `terrain` (into a scene target), `sky-composite`; persistent tables and per-frame products counted in the frame-target share (a second HDR-sized target, 0.7 MB of tables).
- **`render::atmosphere`:** WGSL mirroring the reference in f32 (sky-view and aerial-perspective compute, the composite with the orbit march, the sun disc, five debug views, the terrain's `fs_lit_atmo`), `AtmospherePath`, probes for the oracles; `TerrainRenderer::enable_atmosphere`; the depth is kept (`Store`) and bindable on atmosphere frames; the terrain's atmosphere group is 3 in both tile pipelines.
- **Tests:** tier B oracles against the reference at 1 m, 2 km and 20 km (sky view, aerial perspective, composite), the orbit march at 20,000 km including the limb, the terrain lighting, a 200-frame millimetre walk at 1 m; tier C frames in both tile formats; the graph tests; shader validation and scan.
- **Spec:** ATMO-001 and ATMO-002 active, ATMO-010 and ATMO-011, REND-005 scoped to geometry shaders.

## Expected golden changes:
- sky_orbit_ev15: new, first golden of the sky class
- sky_aerial20km_ev15: new, first golden of the sky class
- sky_ground1m_ev15: new, first golden of the sky class
- sky_ground1m_sunset_ev11: new, first golden of the sky class
- sky_orbit_transmittance_lut: new, first golden of the sky class (a discrete-band view of a CPU-computed table)
- sky_orbit_multiscatter_lut: new, first golden of the sky class (a discrete-band view of a CPU-computed table)

No existing golden changes: plans without an atmosphere draw as before.

## ADR 0037 conditions:
- sky_orbit_ev15: debug-view=sky_orbit_transmittance_lut; bound-tests=the_sky_view_table_matches_the_reference_at_three_altitudes_and_three_suns, the_aerial_perspective_volume_matches_the_reference_at_three_altitudes_and_three_suns, the_orbit_march_matches_the_reference_per_ray_including_the_limb_at_20000_km, the_terrain_lighting_matches_the_reference, the_composite_applies_the_sky_and_the_aerial_perspective_as_the_reference_does, the_composite_from_orbit_marches_each_pixel_as_the_reference_does, a_camera_walking_at_one_metre_in_millimetre_steps_makes_no_step_the_reference_does_not
- sky_aerial20km_ev15: debug-view=sky_orbit_multiscatter_lut; bound-tests=the_sky_view_table_matches_the_reference_at_three_altitudes_and_three_suns, the_aerial_perspective_volume_matches_the_reference_at_three_altitudes_and_three_suns, the_orbit_march_matches_the_reference_per_ray_including_the_limb_at_20000_km, the_terrain_lighting_matches_the_reference, the_composite_applies_the_sky_and_the_aerial_perspective_as_the_reference_does, the_composite_from_orbit_marches_each_pixel_as_the_reference_does, a_camera_walking_at_one_metre_in_millimetre_steps_makes_no_step_the_reference_does_not
- sky_ground1m_ev15: debug-view=sky_orbit_multiscatter_lut; bound-tests=the_sky_view_table_matches_the_reference_at_three_altitudes_and_three_suns, the_aerial_perspective_volume_matches_the_reference_at_three_altitudes_and_three_suns, the_orbit_march_matches_the_reference_per_ray_including_the_limb_at_20000_km, the_terrain_lighting_matches_the_reference, the_composite_applies_the_sky_and_the_aerial_perspective_as_the_reference_does, the_composite_from_orbit_marches_each_pixel_as_the_reference_does, a_camera_walking_at_one_metre_in_millimetre_steps_makes_no_step_the_reference_does_not
- sky_ground1m_sunset_ev11: debug-view=sky_orbit_multiscatter_lut; bound-tests=the_sky_view_table_matches_the_reference_at_three_altitudes_and_three_suns, the_aerial_perspective_volume_matches_the_reference_at_three_altitudes_and_three_suns, the_orbit_march_matches_the_reference_per_ray_including_the_limb_at_20000_km, the_terrain_lighting_matches_the_reference, the_composite_applies_the_sky_and_the_aerial_perspective_as_the_reference_does, the_composite_from_orbit_marches_each_pixel_as_the_reference_does, a_camera_walking_at_one_metre_in_millimetre_steps_makes_no_step_the_reference_does_not
- sky_orbit_transmittance_lut: debug-view=sky_orbit_transmittance_lut; bound-tests=the_sky_view_table_matches_the_reference_at_three_altitudes_and_three_suns
- sky_orbit_multiscatter_lut: debug-view=sky_orbit_multiscatter_lut; bound-tests=the_sky_view_table_matches_the_reference_at_three_altitudes_and_three_suns
