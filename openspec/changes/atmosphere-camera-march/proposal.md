## Why
SPIKE-20 (`spikes/owner-captures-2026-10-20/results/results.md`): the owner sees rings and bands in the atmosphere above about 20 km after #404.
- The aerial-perspective volume (32 slices, 32 rows, trilinear) is too coarse for rays that end 300 to 1,100 km away: between its texels it is 18 to 23 % off at 60 to 99.5 km and 10 % off at 14 km; the bound of ATMO-007 is 5 %, checked only at texel centres.
- The orbit march is within 0.6 % at the same places, so the bands appear only inside the atmosphere.

## What changes
- From 6 km camera altitude inside the atmosphere a pixel with geometry is hazed by a mix of the volume and a march from the camera to the geometry (the smoothstep of the altitude from 6 km to 10 km, so the haze does not step when the camera crosses: opus-planner's order after the first review), above 10 km by the march alone (`FrameGeometry::camera_ray_cosines`, shader `atm_camera_ray`), not by the volume: 8 steps by day, 12 in twilight, dense towards the geometry (`StepLayout::EndDense`), per-channel transmittance as the orbit march has it. At 6 km and below, and for sky pixels, nothing changes. The altitudes (not 20 km) follow from the measurement: the volume is 7.5 to 10 % off from 10 km to 18 km.
- The step counts follow from the accuracy and the cost: the camera march must be within 5 % + 1e-4 of the converged march for every sun and side (the review of the change: at one sun the centre column only looks towards the sun). The sweep over suns 0.7 to -0.08, towards and away, heights 8 to 99.5 km: 6 steps leave 4.8 of the bound, 8 steps 0.9 by day (sun at least 0.1), 12 steps 0.8 in twilight; 8 and 12 are used (`CAMERA_MARCH_STEPS_DAY`, `CAMERA_MARCH_STEPS_TWILIGHT`). Cost (indicative, `adhoc_cost_of_the_atmosphere_passes_at_1440p`, RTX, half of the pixels geometry): 40 steps 6.5 ms, 8 steps about 1.8 ms, 12 steps about 2.6 ms over the volume lookup.
- Budget (ADR 0042 point 5: the atmosphere at 1 ms at 1440p): the sky view, the volume and the composite measure about 0.6 ms at 1 m altitude and about 2.1 ms (by day) to 2.9 ms (twilight) at 20 km with the march (indicative wall times, not timestamp queries); the march alone is over 1 ms. A perf window with the GPU timestamps decides, and a cheaper march (a half-resolution haze buffer) is the follow-up if the budget holds; opus-planner is asked.
- Specs: ATMO-001 (the camera march is matched by the GPU), ATMO-002 (the composite's rule), ATMO-007 (the bounds of the camera march and of the volume between its texels), ATMO-021 (the rule, the across-the-top bound from 10 % to 5 %, measured 3.5 %).
- Tests: tier A (`planet-frame`): the haze continuous across the blend band (the error against the converged march changes by at most 0.48 % per 100 m, bound 1 %); the camera march and the volume between its texels against the converged march; across the top; tier B (`oracle_atmosphere`): the composite above 8 km against the reference; the shader's constants against the reference's.

Expected golden changes:
- sky_aerial20km_ev15: terrain at 20 km altitude is hazed by the march (per channel, no 5 to 10 % volume error): 11,139 pixels on WARP and RTX, max delta 23

Bound tests:
- sky_aerial20km_ev15: oracle_atmosphere::above_6_km_the_composite_marches_from_the_camera_to_the_terrain_and_mixes_it_with_the_volume_as_the_reference_does, atmosphere::above_8_km_the_camera_march_is_within_the_bound_of_the_converged_march_for_every_sun_and_side

The sky goldens are not blessed by the tool: they go to the owner's `/bless`. The pixel-hash tables of `gpu_compact_blocks` (the lit atmosphere frame) are re-recorded for WARP and RTX.

Not covered: terrain past the tangent point (up to 6 % by day, 24 to 39 % in twilight off the converged march), and the volume's own error at a low sun below 8 km (17 to 18 % at 2 km), which the march does not replace; 
