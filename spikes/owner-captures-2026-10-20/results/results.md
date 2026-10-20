# SPIKE-17 bug 2 and SPIKE-20 results: the owner's F9 captures of 2026-10-20

Diagnosis only. Spike code and the failing tests: branch `spike/owner-captures` (never merged). The captures (copied before any build, `target/review/owner-2026-10-20/` of the spikes worktree, not in git): `frame-5161`, `frame-6193`, `frame-13883` (the sea edge), `frame-9825`, `frame-9975` (the bands); each a `.bundle` and a `-path.txt` (the last 60 s of poses). All five bundles record the editor's default world: `single_planet` seed 1, the eroded field (`cpu-erosion-v1`) over a 256-cell in-memory bake, generator 2, mesh 8, definition hash `58100a60bf321ffe` (equal to this build's), 1280x720, tau 3 px, cells 32. The sea-edge bundles are on the RTX (Vulkan).

## SPIKE-17 bug 2: the sea ends at a straight edge: cause found, test fails today

### Answer in one paragraph
**The edge is the 100 km reach of the coastal band (`COAST_REACH_M`, ADR 0056), cut through a plain that is under the sea level and connected to the ocean.** The editor's world has a large plain whose bake elevation is at most 10.9 to 13.5 m (under the 64 m band limit) and whose eroded terrain lies 1 to 10 m under the sea level. It is connected to the ocean through band ground for 120 to 132 km (the bake lattice, 2.5 km grid) and over a 289 km range. `band_water` makes a tile wet when a sample is band **and** an ocean vertex lies within `COAST_REACH_M + 0.71 edge` of the tile centre: a distance test, not a connectivity test. Tiles within 100 km of an ocean vertex draw sea; tiles beyond answer dry, in a straight line along tile borders, over terrain that is below the sea level. Three captures, three places, the same edge.

### Evidence
- `spike17_bundle` replays each bundle (the plan of the frame, then the exact plan of its request, then the path every 25th pose) through `plan_water` with the per-node record of SPIKE-16. The plan of the bundle equals the exact plan of its request (hash equal in all three), so this is not a cover or stale-plan effect.

  | capture | altitude (camera above R) | nodes | `no-water-answer` | gap nodes (terrain under the sea, no water node, within 4 x altitude) | nearest gap node |
  |---|---|---|---|---|---|
  | 5161 | 135.4 m | 1,999 | 1,453 | 885 | level 19, 166 m away, terrain -6.8 m |
  | 6193 | 32.7 m | 2,360 | 222 | 116 | level 21, 39 m away, terrain -0.9 m |
  | 13883 | -2.3 m | 2,579 | 29 | 29 | level 22, 8 m away, terrain -10.0 m |
  No node is cut by the horizon or the height test wrongly (`cut-horizon`, `cut-height-loose` nodes are far or truly above water); every gap is `no-water-answer`.
- The gap tiles in detail (`spike17_tile`): lat 7.52 lon 43.16 (5161), lat 7.51 lon 43.18 (6193), lat 4.55 lon 40.48 (13883): bake continent 0.154 to 0.161 (land), bake elevation 10.7 to 13.5 m (`band_at` true), no ocean vertex of the bake within the reach (`ocean_vertex` false; the nearest at 97.5 to 100 km or none within 100 km along 24 bearings), `water_at` false.
- `spike17_reach`: for the nodes within 3 km of the camera, kept and dry alike have their nearest ocean vertex at 97.5 to 102.5 km (ring resolution 2.5 km): the camera stands on the reach boundary, and the kept/dry split follows it.
- `spike17_flood`: a flood fill on a 600 km tangent grid (2.5 km) around each capture, from the ocean vertices over bake ground at or under the band elevation: **the camera cell is connected to the ocean (path 132, 130 and 120 km)**; 28,900 to 30,500 connected band cells, **16,976 to 18,663 of them (59 to 61 %) farther than 100 km from every ocean vertex, up to 289 km**. The reach drops that part of the plain.
- Earlier SPIKE-17 evidence stays true: the classification is exact within the reach (0 dry tiles of 12,900 near coasts at levels 9 to 16; 1 of about 1,300 inland up to 130 km, at the reach edge); the plain behind the reach is what the sweep's 130 km limit did not reach.

### The test (fails today)
- `crates/app/tests/water_reach.rs`, `a_plain_under_the_sea_level_and_connected_to_the_ocean_through_band_ground_is_wet_beyond_the_100_km_reach` (spec ATMO-019), CPU only, **1.2 s** (the 256-cell bake dominates; `WaterTileCache::tile_water` takes microseconds): the world of the captures, the three gap tiles above (levels 19, 21 and 22, one per capture); each must answer wet. Fails today: `tiles of a plain under the sea level, connected to the ocean through band ground, answer dry: [TileId(1369094539962817300) (level 19), TileId(1513213526671984627) (level 21), TileId(1585283220693825378) (level 22)]`. The patch is `water-reach-test.patch` here. It stays API-neutral, so it holds for any fix of the rule.

### Proposed root fix
Decide the coastal band by **connectivity through band ground, not by distance**: in the ocean channel's computation (`crates/generators/src/ocean.rs`, with the bake) flood from the ocean vertices over bake vertices whose elevation is at most `coast_band_elevation_m(level)` and which are land, and keep that result as a second bit per vertex ("band-connected"); `band_water` then asks "a sample is band and a band-connected vertex lies at the sample" instead of the reach loop (`REACH_PROBES`, up to 2,500 `ocean_bit_at` reads per tile), and the coarse summary (`water_summary.rs`) gets a flag from the same channel. This is the per-vertex sea mask of ADR 0051 option 3 restricted to the band. A basin enclosed by ground above the band elevation stays dry (owner constraint 2 of ADR 0056 holds: the same test, "inland basin stays dry", passes unchanged), and no constant 100 km remains. Costs: the ocean channel and its store key change (re-bake), a spec delta on ATMO-019 and ADR 0056 (an ADR amendment: `COAST_REACH_M` goes), the `water_class_coast_*` and lit coast goldens move where the plain is in view (`golden-review`, escalated classes). A stop-gap, not a fix: raising `COAST_REACH_M` to about 300 km covers this plain (289 km) but floods every basin within that distance of the sea.

## SPIKE-20: the atmosphere still banded after #404: cause found, test fails today

### Answer in one paragraph
**The aerial-perspective volume (32 x 32 x 32, trilinear) is too coarse to hold the haze of rays that end 300 to 1,100 km away, and its error is 21 to 23 % between texels at 60 and 97.5 km altitude (the spec's discretisation bound is 5 %).** #404 stretched the volume's range to the horizon distance to remove the jump at the top; the same 32 slices and 32 rows now sample a haze that changes quickly with the ray (the sky's glow at the horizon, the ground distance running from 300 to 1,100 km over a few degrees), and the linear interpolation between texels gives a ripple: 109 to 127 changes of the error's slope along a 600-ray sweep, which is rings and bands. The orbit march (outside the top) has an error of 0.58 % on the same rays, so the picture changes character at the top as well.

### Evidence
- `between_its_texels_...` (below): the centre column of the volume, 600 rays from 0.95 to -0.95 of the half-height, sun 0.7, land of radiance (0.010, 0.012, 0.006) at the ground distance, the trilinear lookup against the converged march (2,048 steps) of the same ray:

  | altitude | worst error | at | slope changes of the error |
  |---|---|---|---|
  | 20 km | 5.14 % | ground 285 km | 109 |
  | 60 km | 23.09 % | ground 831 km | 127 |
  | 97.5 km | 20.89 % | ground 1,060 km | 111 |
- Where the error comes from (`spike20_error_split`, ignored test): rays through the row centres (so only the distance interpolation between slices matters) 2.1 %, 6.1 %, 8.5 % at 20, 60, 97.5 km; rays halfway between rows (rows and slices) 3.1 %, 10.0 %, 17.5 %. Both directions contribute; the slices at 1,130 km range are about 70 km apart where the ground rays end, the 32 rows are 1.9 degrees apart where the ground distance changes by hundreds of kilometres per degree near the horizon.
- The orbit march at 101, 114, 200 km and 1,000 km (`spike20_orbit_error`, ignored): worst 0.58 %, 0.58 %, 0.58 %, 0.00 % (40 uniform steps from the entry point), so the bands are specific to the inside path: frame 9975 at 97.5 km is just inside the top (the bundle 9825 ends at 114 km, outside; the owner sees the bands where the camera is inside). The ATMO-007 bound is checked only at texel centres and at 1 m, 2 km and 20 km, so nothing measured this.
- Not done: an image of the owner's frame (the bundle holds the terrain plan, not the lit atmosphere frame, and no screenshot was given); the diagnosis rests on the reference arithmetic, which the GPU matches within 1 % (ATMO-001), and on the error's shape, not on a picture. The owner's frame 9975 or a screenshot would confirm that the bands follow the rows and slices.

### The test (fails today)
- `crates/frame/tests/atmosphere.rs`, `between_its_texels_the_aerial_perspective_volume_is_within_the_bound_of_the_converged_march_near_the_top_of_the_atmosphere` (spec ATMO-007), tier A, GPU-free, **0.8 s**: builds the volume at 20, 60 and 97.5 km and compares the trilinear lookup with the converged march along 600 rays; asserts the spec's 5 % (`MARCH_REL`). Fails today: `the trilinear aerial-perspective volume differs from the converged march by up to 23.1 %`. The patch is `ap-between-texels-test.patch` (with the two ignored diagnostics).

### Proposed root fix
1. **Do not use the volume where it cannot hold the haze.** For a camera above about 20 km, composite the geometry pixel with a per-pixel march from the camera to the geometry (the orbit march's code, started at the camera, 40 uniform steps: 0.58 % against the converged march) instead of the volume lookup; below 20 km keep the volume (5.1 % at 20 km, 3.1 % at 2 km: bring 20 km under 5 % with a finer slice law or 64 slices). This makes the inside and the outside path the same arithmetic at the top, so the continuity of #404 holds by construction and the ripple goes. Cost: the pixel march for the covered pixels above 20 km (the orbit path already pays it for the whole frame).
2. Or enlarge the volume (64 slices and 64 rows: 8 x the froxels, 8 x the memory and the AP pass), and add the test above at 60 and 97.5 km as the gate. Not recommended: the error is smooth only if the volume follows the ground distance, which it does not.
Costs of 1: a spec delta on ATMO-001, ATMO-007 and ATMO-021 (the range), `numerics-reviewer` (shader), the lit orbit and aerial goldens at altitudes above 20 km move (escalated classes).

## Files (spike branch `spike/owner-captures`)
`crates/app/examples/spike17_bundle.rs`, `spike17_tile.rs`, `spike17_reach.rs`, `spike17_flood.rs` (the capture analysis); `crates/frame/src/water.rs` (the per-node record of SPIKE-16, re-applied); `crates/app/tests/water_reach.rs` and `it.rs` (the water test); `crates/frame/tests/atmosphere.rs` (the atmosphere tests).
