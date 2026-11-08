# Prototype region and fixtures E1-E6

Status: complete 2026-11-08 (inbox #100; a task of SPIKE-105 §13.3 and `docs/design/terrain-target.md` §12.2).
Method: `crates/cache/examples/prototype_region.rs` on `spike/terrain-pipeline-review` (never merged), by a Sonnet sub-agent. Grid: 3,072 cells a face edge (3.26 km; cell area 10.6 km^2), elevation from `BakeInputs::base` (seed 14, the current code), driver fields from the same inputs on a stride of 6 cells (19.5 km). All six faces routed as one graph across cube edges (F6's method). Raw output: `region-search-output.txt`. Coordinates are (i, j) on that grid.

A first run read the store `global16384-m9c14`, which is older than the current code (a mean difference of 1,688 m against the base; F6 read the same store). Those coordinates were discarded.

## Selection

Window rule: land 40-90 %, mountains + high mountains at least 15 %, plateau at least 3 %, a river over 3,000 km^2 reaching the sea inside the window, at least 3 tributaries over 500 km^2, at least one closed basin. 94 of the windows on a 125 km grid pass. Soft: mid-latitude +1, polar -3.

| # | Face | i | j | Centre (lat, lon) | Land | Mt + high | Plateau | River (km^2) | Tribs | Basins |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 1 | 2014-2168 | 190-344 | -36.0, -163.7 | 70 % | 26 % | 12 % | 591,555 | 17 | 14 |
| 2 | 5 | 228-382 | 532-686 | -48.3, 144.8 | 77 % | 17 % | 12 % | 78,107 | 11 | 7 |
| 3 | 1 | 1824-1978 | 304-458 | -33.4, -169.3 | 74 % | 23 % | 6 % | 115,112 | 19 | 8 |
| 4 | 5 | 2584-2738 | 950-1104 | -55.0, 22.3 | 78 % | 27 % | 35 % | 30,383 | 9 | 6 |
| 5 | 1 | 342-496 | 1216-1370 | -6.0, 147.3 | 72 % | 22 % | 16 % | 38,067 | 10 | 13 |

**Chosen: #1**, face 1, i 2014-2168, j 190-344; alternate: #2 if a river wholly inside the window is preferred.

Caveats on the chosen window:
- The main river's catchment (591,555 km^2) is larger than the window (250,000 km^2). The solver's scope is the landmass (A3 §6.1), so this costs nothing; only the local refinement (K7) is limited to the window.
- Basin counts include coastal depressions with floors at the 1 m land clamp (`MIN_LAND_M`, GEN-098): 9,031 km^2 at depth 615 m with its floor at z = 1 m. They are generator artefacts of the centred band beside the coast (noted for the class-source tuning, not used as "tectonic").
- No passing window on the planet has an arid closed basin (aridity under 0.3). The arid sink (ADR 0105 point 2) cannot be exercised on this seed's windows: a synthetic fixture covers it (below).
- Precipitation in the chosen window is reasonable (land p50 716, p95 1,445 mm/yr). The equatorial candidate reaches over 10,000 mm/yr at orographic heads (`orography().precipitation_mm`, unclamped; the field bake clamps at 4,500 mm): noted for the climate tuning.

## Edit fixtures

All on face 1. Cells on the 3.26 km grid; convert by `k` for finer grids.

| Case | Location | Parameters |
|---|---|---|
| **E1** Raise a range | block i 2153-2168, j 195-210, centre (2160, 202), -37.62, -161.70 | raise by 1,000 m over about 50 x 50 km; mean z 1,370 m, mountain weight 1.0 |
| **E2** Move a divide | divide between mouths (2048, 322), 41,696 km^2, and (2041, 332), 11,833 km^2; 137 km of shared border. Mid (2024, 307), z 602 m; lowest pass (2031, 311), z 526 m | raise a ridge line (2012, 298) to (2037, 316), or lower the pass by 100 m, so the catchments trade area |
| **E3a** Give a basin an outlet | basin at lowest (2082, 216), spill (2086, 221) at 616 m, beyond 591 m; 9,031 km^2 | cut 25 m at the spill: the basin gains an outlet (class changes, id stays) |
| **E3b** Merge | the same basin and its neighbour, lowest (2095, 224), spill 518 m; 50 km apart | lower the sill between them so one spill level joins them; check the merged id and the continuity map |
| **E3c** Split | the merged basin of E3b | raise a sill across it (the reverse): new ids by §5.4 |
| **E3d** Intentional basin | lowest (2155, 205), z 457 m, spill (2145, 211) at 1,187 m; 1,072 km^2; in the range | keep as class intentional (tectonic), through every edit |
| **E4** Far downstream | trunk at (2097, 239), area 480,666 km^2, 164 km from the mouth; near-mouth (2095, 271), area 590,748 km^2 | raise a 50 x 50 km block upstream of (2097, 239) by 500 m, or change its runoff; check discharge and dirty set at the mouth |
| **E5** Rainfall only | tributary head (2051, 261), z 643 m, joining at (2086, 263), area 1,942 km^2; rain window i 2036-2067, j 246-277; mean 632 mm/yr | double the precipitation over the window: initiation moves with no height change |
| **E6** Rock and uplift | block i 2136-2167, j 210-241, centre (2151, 225), -37.0, -161.97; mountain weight 0.99, uplift 24.3, hardness 0.29 | a softer lithology (hardness 0.1) and, separately, uplift x 2 |

## Cube-edge window

Face 1 i 2086-2117, j 0-16 and face 5 i 0-16, j 2085-2116 (-43.8, -163.4), 856 km from the chosen window. River of 8,012 km^2 crossing the face 1 / face 5 edge: upstream (face 1) (2101, 0), z 752 m, area 8,012 km^2; downstream (face 5) (0, 2100), area 8,023 km^2. Alternates on the same edge: 6,675 km^2 at face 1 (2214, 0), 5,094 km^2 at face 1 (2228, 0). 357 land-to-land crossings carry over 3,000 km^2; the largest is 909,799 km^2.

## Synthetic fixtures

Analytic heights on small grids (S105 §13.3, the first review's suite), each with the property a test asserts:

| Fixture | Definition | Asserted |
|---|---|---|
| Cone | one cone, radial | radial drainage; area conservation at the outlets (true cell areas, G-CON) |
| Ridge | two slopes meeting at a ridge | one divide, on the ridge; G-LOCAL |
| Plane | a plane with a gentle tilt | stable routing, no directional artefacts along the grid axes (MFD, F5's metric) |
| Closed basin (endorheic) | a bowl with no outlet and aridity below the threshold | class endorheic; a sink; no outflow (covers the arid case the planet lacks) |
| Lake with outlet | a bowl with one sill | class lake; spill and outlet right; connects downstream |
| Mountain river | tilted plane with a parabolic channel and tributaries | connected tributaries; segments and confluences |
| Coastal river | a plane to a sea | outlet at the sea |
| Cube-edge crossing | a river across a cube edge | one chain of segments across the edge; area conserved |
| Multiple catchments | several bowls and rivers side by side | the downstream hierarchy right; a catchment's round independent of the others (G-LOCAL, G-ROUND) |
| Merge and split | two bowls with a sill raised and lowered | ids and continuity maps per A3 §5.4 (G-BASIN) |
