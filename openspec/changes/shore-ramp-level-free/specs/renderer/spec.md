## MODIFIED Requirements

### Requirement: REND-030 The shore ramp follows the mesh's tolerance
The lit terrain's albedo goes from the seabed to the shore over a ramp of height above the water. The ramp at a fragment SHALL be `max(shore_blend_m, shore_ramp_at(per_m, d))` tall, where `d` is the fragment's distance from the camera, `per_m = SHORE_RAMP_K * tau_px * pixel_angle` with `pixel_angle = 2 tan(fov_y / 2) / viewport_h_px` (`LodParams::shore_ramp_per_m`, carried once per plan as `TerrainPlan::shore_ramp_per_m` and in the plan text as `shore_ramp_per_m`; a plan without it reads 0 and ramps by the look's blend alone) and `shore_ramp_at(per_m, d) = min(per_m * d, SHORE_RAMP_MAX_M)` (`planet_frame::lod::shore_ramp_at`, 8 m cap). The ramp SHALL be a function of the fragment's distance and of nothing about the node that draws it: the same ground point has the same ramp, and so the same albedo, under a tile of any level (the cover, stale plans and prefetched tiles draw tiles the exact plan would not; a ramp that depended on the drawn level stepped the albedo of the whole shallow seabed at every tile border between levels). The shader forms `d` as the length of the node's camera-relative origin plus the fragment's tile-local position (camera-relative f32, no planet-radius value) and reads `per_m` from the globals; the same width SHALL enter the f64 reference `AlbedoParams::albedo_with_ramp` and the shader's `albedo_of`, so that `oracle_lit_shading` pins them together.

Derivation of the width (no look number, no screen derivative). A node is drawn only closer than its parent's split distance, where the parent's sample spacing projects to `tau_px` pixels: `tau_px * pixel_angle * split(level - 1) = s(level - 1)` whatever the field of view, the viewport and `tau_px` are. So `tau_px * pixel_angle * d` is the world size of the selection's screen-space tolerance at distance `d`, and the height of one ground point differs between a node's mesh, its parent's mesh and the morph between them by a fraction of it. A ramp narrower than that makes an albedo that differs by a factor of eight in luminance between seabed and beach (0.06 against 0.52) flash with decimetre changes of height between frames. `SHORE_RAMP_K` is the smallest factor found that holds the terrain's own shading flash at the shore of both coastal flights under 0.5 %: on the flights of `gpu_water` (a 1:50 beach, tau 4 px) k = 1 leaves 4.3 % of the glide's shore pixels flashing, k = 1.5, 2 and 3 leave none; 2 is taken. At the far end of a node's drawing range the ramp is `K s(level - 1)` (up to the cap), at the near end half of it; k = 2 was calibrated with the former per-node rule, which blended from `K s(level - 1)` to twice that across the morph, so the distance rule is about 0.5 to 0.7 of it pointwise, and the flights (the scenario "No flash on the coastal flights") are the evidence for it, not the derivation. The cap of 8 m is four absorption lengths of the water pass (0.5 per metre by default: 98 % absorbed), below which the seabed's own colour is hidden whatever the ramp is; the flights need 3 m; without the cap the coarse view of an orbit asks for kilometres and the deep seabed takes the beach's colour in terrain-only views. Near the camera `tau_px * pixel_angle * d` is centimetres and the ramp stays at the look's `shore_blend_m`, 1 m by default; it widens only where the mesh cannot hold 1 m of height. The seabed and the beach colours, the beach band above the water, and every other term of the albedo are unchanged.

Verify: A, B, C
Status: active
Source: M5 gate tracked item "terrain shading at the beach flashes" (6.3 % of the shore-band pixels of the glide and 36.8 % of the descent flashed by more than a quarter between frames with no water drawn), the attribution of 2026-10-18 (one term: the 1 m seabed-to-shore ramp), ADR 0039 point 3, REND-026, SPIKE-16 (the level-dependent ramp as the cause of the dark patches while the camera moves)

#### Scenario: The width
- **WHEN** the tolerance is 0, 0.3, 7.5 m or unreadable
- **THEN** the ramp is the look's blend (1 m), the blend, 7.5 m and the blend; the width never falls below the blend and never shrinks as the tolerance grows

#### Scenario: The node's tolerance
- **WHEN** the ramp is taken at the far end of the drawing range of a node of any level 1 to 22 (the parent's split distance), for two different tau, viewport and field-of-view settings
- **THEN** it is `SHORE_RAMP_K` times the parent's sample spacing up to 8 m, for both settings, `per_m` is `SHORE_RAMP_K * tau_px * pixel_angle`, the ramp is 0 at the camera and 8 m far away, and at 40 m it is below the blend

#### Scenario: Continuous across a level border
- **WHEN** the albedo of a seabed point 2.5 m under the water is taken at a fragment distance from 5 m to 20 km as a tile of any level 16 to 22 would draw it, and the ramp is walked along the ground in 5 m steps
- **THEN** the albedo is the same whichever level draws it (under the former per-level ramp it was 4.9 times brighter under L17 than under L19), and the ramp grows with the distance by at most `per_m` per metre without a step

#### Scenario: Shader and reference
- **WHEN** probes sit at the foot, the middle and the top of a ramp of 8 m and at fragments with random distances and growths up to the cap
- **THEN** the shader's radiance equals the reference's within the declared bound of `oracle_lit_shading`

#### Scenario: No flash on the coastal flights
- **WHEN** the glide and the descent over a 1:50 beach are drawn with the plan's ramp
- **THEN** at most 0.5 % of the shore-band pixels change their terrain-only luminance by more than a quarter between three consecutive frames
