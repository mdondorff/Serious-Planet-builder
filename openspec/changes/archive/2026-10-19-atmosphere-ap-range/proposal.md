## Why
SPIKE-17 bug 1 (the owner's descent video, 0:26 to 0:28): terrain farther than 256 km along its ray is composited by two formulas, one inside the atmosphere top (100 km) and one outside, and they disagree by about 95 % in the blue channel. Inside, `atm_ap_lookup` clamps the distance to the last slice of the aerial-perspective volume, `AP_MAX_DISTANCE_M` = 256 km, so land at the horizon (up to 1,130 km away) gets the haze of 256 km; outside, the orbit march integrates the whole ray. A camera hovering around 100 km flips between the two (`spikes/descent-video-bugs/results/results.md`, the failing tier-A test `terrain_beyond_the_aerial_perspective_range...`).

## What Changes
- **The volume's range is per frame** (ATMO-021): `range = max(256 km, sqrt(c))`, `c = h (2R + h)` the camera's tangent distance squared, so 256 km up to about 5 km altitude (every near-ground frame is as it was) and the distance to the horizon above it (505 km at 20 km, 1,130 km at the top). The slices stay quadratic in the range (`range ((k + 1) / 32)^2`). The CPU sets `FrameGeometry::ap_range` and sends it in the word the GPU already reads as the range (`misc.z`); the shader changes nowhere (`atm_ap_slice_distance`, `atm_ap_lookup` and the sky pixels' quarter-range lookup read `A.misc.z`).
- **Tests**: the SPIKE-17 test (inside the top equals outside within 10 %: 95 % before, 7.2 % now: the 32 slices at the coarser spacing, about 70 km apart near 300 km), a test that the range is exactly 256 km up to 5 km and the horizon above, reaches the GPU word and the slice distances follow it, and the ATMO-007 march bound at h = 60 and 99 km (passes).

## Impact
- Specs: ATMO-021 (added, atmosphere-ocean); ATMO-007's bound is measured at two more altitudes.
- Code: `frame` (`look/atmosphere.rs`: `ap_range`, `ap_slice_distance_for`, `ap_texel`, `ap_lookup`, the GPU word), tests.
- Goldens: the lit frames with the camera above 5 km altitude move (the aerial 20 km views and any view at that height; the orbit views are outside the atmosphere and the volume is not used there): `golden-review`; lit and sky classes escalate to the owner's `/bless`.
- Perf: no new pass; the march step count per slice is unchanged (a longer step at the top).
- numerics-reviewer (atmosphere arithmetic); spec-guardian before archive.

Expected golden changes:
- sky_aerial20km_ev15: the camera at 20 km altitude has a 505 km aerial-perspective range instead of 256 km (ATMO-021): distant terrain and the sky pixels' quarter-range lookup are hazed to the longer range (declared, lit and sky class).
- the recorded frame hashes of `gpu_compact_blocks::persistent_renderers_per_slot_and_batched_and_the_lit_atmosphere_frame_are_the_recorded_pixels` (the lit atmosphere frame at the aerial height) on WARP and the RTX: same cause.

Bound tests:
- sky_aerial20km_ev15: atmosphere::terrain_beyond_the_aerial_perspective_range_is_hazed_the_same_just_inside_and_just_outside_the_top, atmosphere::the_aerial_perspective_range_is_256_km_up_to_5_km_altitude_the_horizon_distance_above_it_and_reaches_the_gpu_words, atmosphere::the_aerial_perspective_march_is_within_the_bound_of_the_converged_march
