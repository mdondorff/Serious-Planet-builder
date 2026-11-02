Opened: 2026-11-01
Question: Which slopes should the heightfield terrain be judged on, given that cliffs and the steepest mountain faces need another technology (owner, 2026-11-01)?
Why it matters: the owner discusses cliffs on 2026-11-01 evening; M9 criterion 1 (TEST-081), the bake's thermal rule (GEN-101) and the LOD gate (TEST-083) all touch slope.
Time box: 2 h (analysis only, Sonnet)
Success criteria: an inventory of every slope target; GLO-30 slope shares above 35/45/55/65 degrees at the four criterion-1 sites at heightfield spacings, and the share of 1 km relief that needs those slopes; what the LOD engine renders without caps; a proposed ceiling with the targets it changes and candidate technologies for the rest.
Fallback if it fails: the inventory and the GLO-30 numbers alone.
Code location: Python scripts in `proto/` (run from a scratch directory, read only on `C:/DEV/SeriousPlanetData/dem/glo30-samples/`); no branch, no Rust.
Closed: 2026-11-01

**Licence notice (Copernicus WorldDEM-30, Art. 6).** The statistics in this folder are produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. No endorsement by Airbus, ESA, the EU or Copernicus is implied. Raw tiles are not in the repository. Citation: https://doi.org/10.5270/ESA-c5d3d65.

## Headline
- **Slopes above 45 degrees carry none of the 1 km relief that criterion 1 judges**: limiting the real terrain to 45 degrees keeps the 1 km relief p50 at 0.997-1.000 at all four sites (Himalaya 0.997). Even a 35 degree limit keeps 0.985 in the Alps.
- **Steep area is an Alps and Himalaya matter**: at 30 m, 4.3 % of the Alps tile and 8.7 % of the Everest tile lie above 45 degrees; the three hill sites have 0.004-0.12 %.
- **The steep share shrinks with grid spacing**: the Alps above 45 degrees fall from 4.3 % (30 m) to 2.2 % (100 m, the bake layer) and 0 % (600 m, the global store).
- **The LOD engine has no slope limit of its own**: capping comes from loose height bounds (SPIKE-82); with the min/max pyramid the steep field plans to level 18 with no capped tile (SPIKE-83). What limits slope is the data: the bake's thermal rule (33 degrees, up to about 41 with its 25 % perturbation; GEN-101) at 101.8 m.
- **Proposal**: a heightfield slope ceiling of **45 degrees at 30 m**. Criterion 1 then judges the slope p50 over cells at or below 45 degrees on both sides; the Alps' real reference drops from 23.7 to 22.5 degrees (0.95), the hill sites' by under 0.2 %; the relief ratio is unchanged. The share above 45 degrees goes to a cliff technology, first triplanar shading plus steep-face displacement, then instanced cliff meshes.

## Method
- Real tiles: GLO-30 1 degree tiles (Alps N47E011, Highlands N57W005, Black Forest N48E008, Valley and Ridge N40W078; Everest N27E086 as a reference), decoded with SPIKE-55's reader (`proto/glo30.py.txt`).
- Spacings: block means of the 1 arc-second grid to about 30, 60, 100, 200 and 600 m (100 m: the bake layer, 101.8 m; 600 m: `global16384`). LOD vertex spacing per level (R 6,371 km, 32 cells): level 13 38.2 m, 14 19.1 m, 15 9.5 m, 18 1.2 m, 22 7.5 cm; levels finer than 30 m inherit their slopes from the data below them.
- Slope: atan of the central-difference gradient (as `descriptors::slope_degrees`, GEN-089).
- Relief carried by steep slopes, two measures:
  - slope-limited surface: m = (lower + upper envelope)/2 with lower = min over y of h(y) + tan(c)·|x - y| (8-neighbour chamfer; upper symmetric); m has slopes at or below the ceiling c and equals h where h already does. On a synthetic 300 m vertical cliff it keeps the full height range and spreads it over 580 m at 45 degrees (it flattens more than needed, so the relief loss is an upper bound). The lower envelope alone (peaks cut, slope exactly c) gives the same ratios within 0.01.
  - rise share: the share of the summed gradient magnitude (total ascent per area) on cells above c.
- Relief: 1 km non-overlapping blocks, p50 (as SPIKE-55 and the scorecard).
- Criterion-1 window: "centre" is the central half of the tile in each axis (0.5 degree), as TEST-079 uses; the values differ by up to 1.4 degrees from SPIKE-74's resampling (Alps 23.7 against 22.35); ratios are what counts.
- Inventory: ADR 0088 and 0089, `openspec/changes/m9-bake/specs/`, SPIKE-74 to 83, `docs/briefings/m9-draft.md`, `crates/app/src/terrainscore.rs`.

## 1. Slope targets in force or planned
| Number | Where | What it measures | Kind |
|---|---|---|---|
| >= 0.6 of real at both sites | `docs/briefings/m9-draft.md` criterion 1; TEST-081 (`openspec/changes/m9-bake/specs/testing/spec.md`) | generator p50 / real p50 of slope (degrees, 30 m) and of 1 km relief at Valley and Ridge, Black Forest, Highlands, Alps | pass/fail; a floor, no upper bound |
| 0.8 of real | TEST-081; m9-draft R7 | the same ratios | tuning aim |
| below 0.4 after the bake and 2 tuning rounds | m9-draft criterion 1 | the same ratios | stop rule |
| not worse than the baseline | m9-draft criterion 1 | Badlands and Erg Chebbi descriptor distances (slope among them) | pass/fail |
| 0.65 / 0.55 gives FAIL | TEST-081 scenario | slope / relief ratio | test example |
| 33 degrees, ±25 %, 1/16 of the excess per pass | GEN-101 | thermal critical slope in pass 2 | generator rule; the only slope limit in the bake |
| 9,000 m | GEN-096 | elevation clamp | height, not slope |
| k 1 / 5 / 6 / 7.5; step <= 2.0 per lattice cell | GEN-097, TEST-082 | amplitude per class | start values; affects slope through k |
| hill band 30 / 175 / 825 m | GEN-098 | band amplitude at k 1 / 5 / 7.5 | start values |
| 0 capped tiles, deepest level as today, <= 1.1x nodes | TEST-083 | LOD plans at high-mountain, mountain and hill poses, 200 m and 2 km | pass/fail; bound-driven, not slope-driven |
| Wasserstein-1 over p1-p99 / real p5-p95 | TEST-079, `terrain-score` | slope distribution distance per landform | reported; p99 of the Alps (54 degrees) and Everest (58) lies above 45 |
| real slope p50 Alps 22.35, Everest 29.14 (central 0.5 degree) | SPIKE-74 | reference values | measured |
| generator slope p50 Himalaya 24.5-31.3, about 33 in SPIKE-80 | SPIKE-74, SPIKE-80 | pipeline output with thermal 33 | measured, stand-in |
| "slope p50 up to 31 degrees will cap more tiles" | SPIKE-76 | risk statement | resolved by SPIKE-82/83 (bounds, not slope) |

## 2. GLO-30 slope distribution
Share of area above each angle (fraction) and slope p50/p99 (degrees), by grid spacing.
| Site | Spacing | p50 / p99 | > 35 | > 45 | > 55 | > 65 |
|---|---|---|---|---|---|---|
| Alps (Tyrol) | 30 m | 19.3 / 54.4 | 0.166 | 0.043 | 0.009 | 0.001 |
| | 100 m | 16.9 / 49.0 | 0.114 | 0.022 | 0.002 | 0.000 |
| | 200 m | 15.0 / 44.7 | 0.072 | 0.009 | 0.001 | 0.000 |
| | 600 m | 10.3 / 34.9 | 0.010 | 0.000 | 0.000 | 0.000 |
| Scottish Highlands | 30 m | 7.2 / 34.2 | 0.009 | 0.001 | 0.000 | 0.000 |
| | 100 m | 6.3 / 30.8 | 0.004 | 0.000 | 0.000 | 0.000 |
| Black Forest | 30 m | 7.3 / 36.6 | 0.015 | 0.001 | 0.000 | 0.000 |
| | 100 m | 5.4 / 28.4 | 0.000 | 0.000 | 0.000 | 0.000 |
| Valley and Ridge | 30 m | 7.2 / 29.3 | 0.001 | 0.000 | 0.000 | 0.000 |
| | 100 m | 5.7 / 25.6 | 0.000 | 0.000 | 0.000 | 0.000 |
| Everest (reference) | 30 m | 29.3 / 58.2 | 0.312 | 0.087 | 0.018 | 0.002 |
| | 100 m | 26.3 / 53.7 | 0.209 | 0.046 | 0.008 | 0.001 |
| | 600 m | 17.3 / 40.2 | 0.029 | 0.003 | 0.000 | 0.000 |
All spacings for all sites: `raw/s85_slope.log.txt`. GLO-30 is a surface model: canopy edges and buildings add a few steep cells at the hill sites (an upper bound there).

### How much of the relief needs the steep slopes (30 m)
| Site | 1 km relief p50 | ratio kept at ceiling 25 / 30 / 35 / 45 / 55 | rise share above 30 / 35 / 45 / 55 |
|---|---|---|---|
| Alps | 366 m | 0.89 / 0.97 / 0.99 / 1.00 / 1.00 | 0.57 / 0.39 / 0.14 / 0.04 |
| Highlands | 133 m | 1.00 / 1.00 / 1.00 / 1.00 / 1.00 | 0.09 / 0.05 / 0.008 / 0.001 |
| Black Forest | 106 m | 1.00 / 1.00 / 1.00 / 1.00 / 1.00 | 0.15 / 0.06 / 0.003 / 0.000 |
| Valley and Ridge | 107 m | 1.00 / 1.00 / 1.00 / 1.00 / 1.00 | 0.03 / 0.006 / 0.000 / 0.000 |
| Everest | 516 m | 0.82 / 0.92 / 0.97 / 1.00 / 1.00 | 0.67 / 0.50 / 0.19 / 0.05 |
- The 1 km relief survives a ceiling far below the real tail: the blocks' height range comes from long moderate slopes; steep cells are short.
- The rise share is larger: 14 % of the Alps' total ascent and 19 % of Everest's sits on faces above 45 degrees. That is what a viewer sees as cliffs and rock walls; it is visible, not measurable by criterion 1.
- At 100 m (the bake layer) the 1 km relief p50 keeps 0.986 (Alps) and 0.976 (Everest) at 35 degrees, 1.00 at 45 (`raw/s85_slope.json`).

## 3. What the LOD engine renders
- Production parameters: 32 cells, tau 6 px, morph band 0.3, finest level 22 (LOD-010).
- Capping is bound-driven (SPIKE-82 finding 2): morph room needs split + 2ρ under the parent's split distance, ρ from the neighbours' height bounds. Today's style of bounds caps the steep combined field at level 13 (Himalaya, 62 capped) and 15 (Alps, 85 capped); near-exact bounds and the min/max pyramid plan both to level 18 with 0 capped (SPIKE-83: 1,075 and 1,094 nodes at 200 m).
- Neither SPIKE-82 nor 83 measured slope statistics of the steep field (k-scaled today's field without carve or thermal); its slope p50 is not known. The closest proxies are the pipeline's Himalaya p50 of 24.5-31.3 (SPIKE-74) and about 33 degrees (SPIKE-80).
- The spec's capping stress case, 220 m over 100 m (about 66 degrees), caps (terrain-lod spec).
- Level steps grow with k: Himalaya steep L14 p99 4.0 m against 0.62 m today; L18 0.22 m (SPIKE-82).
- Answer: rendered without caps, with the pyramid: the steep stand-in at mountain poses to level 18 (1.2 m spacing); no slope figure is attached to it. The data, not the engine, sets the slope: the bake's thermal rule holds pass-2 slopes near 33 degrees (at most about 41 with the perturbation) at 101.8 m, and the global store at 600 m holds at most 47 degrees even for the real Alps.

## 4. Proposal
### Ceiling
- **45 degrees, measured at 30 m.** It keeps all of the 1 km relief at the four sites (0.997-1.000), covers 95.7 % of the Alps tile and over 99.8 % of the hill tiles, and is above the bake's thermal range (33-41 degrees), so the heightfield's own data never needs to exceed it.
- 35 degrees would cut 16.6 % of the Alps (39 % of its ascent) and conflict with the real p50 of 22-24 degrees on the Alps' central window; 55 degrees would leave slopes the heightfield draws badly (texture stretch, level steps of several metres at level 14) inside the targets.

### Targets that change if criterion 1 judges only slopes at or below 45 degrees
| Target | Today | With the ceiling |
|---|---|---|
| TEST-081 slope ratio | generator p50 / real p50, all cells | p50 over cells <= 45 degrees on both sides |
| real slope p50, Alps (central window) | 23.7 (SPIKE-74: 22.35) | 22.5 (0.949 of today; SPIKE-74 scale: about 21.2) |
| real slope p50, Highlands / Black Forest / Valley and Ridge | 6.81 / 9.33 / 8.34 | 6.81 / 9.32 / 8.34 (under 0.2 %) |
| real slope p50, Everest (not a criterion-1 site) | 29.3 | 28.4 |
| TEST-081 relief ratio | 1 km relief p50 | unchanged (the ceiling keeps 0.997-1.000) |
| TEST-079 slope distance | W1 over p1-p99 | over p1 to the 45 degree point (Alps p99 54, Everest 58 lie above) |
| GEN-101 thermal 33 ±25 % | the only slope limit | unchanged; consistent (below the ceiling). R7 may raise it per rock class towards 40-45 (bedrock; talus is about 33-35) |
| TEST-083 LOD gate | 0 capped, bound-driven | unchanged |
| TEST-082, GEN-096 to 098 | amplitude and steps | unchanged |
- A new target follows for the other technology: the share of area above 45 degrees per landform (Alps about 4 %, Himalaya about 9 % at 30 m) as its coverage goal.

### Candidate technologies for the share above 45 degrees
| Technology | Fit | Cost |
|---|---|---|
| Triplanar shading on steep faces | removes texture stretch on any slope; no new geometry, no overhangs | low: shader only, about 3x the ground texture samples on steep pixels |
| Vertical displacement on steep faces (along the normal, finest levels) | sharpens 45-75 degree faces from a baked steepness mask; single-valued, so no overhangs | low to medium: shader and a mask channel; bounds must include the displacement (SPIKE-83 pyramid) |
| Instanced cliff meshes (rock-wall kits on a baked cliff mask) | true vertical faces, ledges and overhangs; reuses SPIKE-28's GPU instancing (about 0.45-0.65 ms per million visible triangles) | medium to high: mesh kits or procedural meshes, placement bake, mesh LODs, seams to the heightfield, collision |
| Voxel patches (local SDF, dual contouring) | overhangs, arches, caves; editable | high: a new subsystem with meshing, LOD seams, storage and editing; the risk profile of the chunk stage (ADR 0086) |
- Order of fit: triplanar and displacement first (inside the heightfield, no new subsystem), instanced cliff meshes next (an existing measured path), voxel patches only if caves or overhangs become a goal.
- Risk (lesson of the chunk stage): whichever layer is chosen must be pre-baked from the heightfield's own steepness mask and checked by a gate in its first PR, not discovered in live tests.

## Limits
- One tile per landform; the Alps tile mixes valleys and high relief (its central window is steeper).
- GLO-30 is a DSM; canopy and buildings add steep cells at the hill sites (they are under 0.2 % above 45 degrees there).
- 30 m is the finest real spacing available; slopes at LOD levels finer than 30 m (14 to 22) are not measured.
- The slope-limited surface over-flattens pure cliffs (a 45 degree ceiling spreads a vertical cliff at about 27 degrees mean), so the relief ratios are lower bounds of what a ceiling keeps.
- The steep field's slopes in SPIKE-82/83 are not measured; the LOD statement is about bounds.

## Visuals
Nothing to look at: tables of statistics only (no image derived from GLO-30 is in the repository).
