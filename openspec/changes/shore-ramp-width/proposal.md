## Why
The tracked "terrain shading at the beach flashes": the terrain's own shading at the shore flashed by more than a quarter between frames at 6.3 % (glide) and 36.8 % (descent) of the shore-band pixels. Attributed to one term: the 1 m seabed-to-shore albedo ramp, narrower than the mesh's height tolerance (REND-030).

## What changes
- Each plan node carries `shore_ramp_min_m` = `min(SHORE_RAMP_K * sample_spacing(level - 1), 8 m)`; the lit shader and the f64 reference use `max(shore_blend_m, shore_ramp_min_m)` as the ramp. Near the camera nothing changes (1 m).
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
