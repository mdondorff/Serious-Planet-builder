# SPIKE-82: steeper terrain and the LOD engine

Opened: 2026-10-31
Question: the combined generator (SPIKE-80/81) makes terrain much steeper (`k` up to 7.5, a class hill band up to 825 m, belts to 8 km). What does that do to the LOD engine before R4 commits? Capped tiles (ADR 0074), level steps (tile against parent), bounds tightness and node counts at production parameters, against today's field at the same poses; and if caps or steps blow up, the lever and its cost.
Why it matters: M9 criterion 6 (no degradation) and the "re-check" verdicts on ADR 0014, 0016 and 0074 (SPIKE-76).
Time box: 2.5 h (opus-planner, 2026-10-31); used about 1 h 15 min, with 3 min of machine time.
Success criteria:
- plans (nodes, capped, deepest level) at hill, Alps and Himalaya poses, 200 m and 2 km, steep against today;
- level steps p99 and max per level;
- bound tightness;
- the lever and its cost.
Code location: branch `spike/m9-plains` (commit 096e234d), the ignored test `spike82_lod` in `crates/app/src/plancheck.rs` (`proto/spike82_lod.rs.txt`), with spike accessors `ErosionField::spike82_base_bounds` and `global::spike82_hill_pattern_bounds`. Spike code, never merged.
Closed: 2026-10-31

## Method
- **The steep field** (`Steep`, a `HeightField`):
  - height: `b + w Delta + (1 + w (k - 1)) (e - b) + w hb(k) p`, where:
    - `b` is the bake (`base_elevation`) and `e` today's eroded field (the field of `generate --plan-check`);
    - `Delta` (the combined elevation minus today's) and `k` come from SPIKE-81's derived-class fields (0.25 degree, bilinear);
    - `p` is today's 2-8 km ridged pattern with SPIKE-80's class hill band `hb(k)`;
    - `w = smoothstep(0, 400 m)` of `b` is the coast taper.
  - It is a point function with per-cap bounds, as ADR 0088's baked channels would be. SPIKE-66 showed that a runtime stencil cannot plan.
- **Bounds per cap** by interval arithmetic:
  - the bake's scan range and the eroded field's own cap bounds;
  - for `Delta` and `k`, the range of the 0.25-degree cells the cap touches, intersected with the centre value plus or minus the bilinear gradient bound times the radius;
  - for the hill band, the per-octave noise bounds at the warped centre (the warp taken as at most 5.5 times Lipschitz, a spike assumption).
  - The first run, with the cell scan two cells wider than the cap, gave bounds up to 31,000 times the sampled range and capped plans everywhere (`raw/run1-loose-bounds.log`). The bounds are the result's main sensitivity.
- **Variants:** `today` (the eroded field); `steep-nohb` (no hill band); `steep` (with it); and as a lever test, `today-tight` and `steep-tight`. The tight variants take near-exact bounds: the range of 17 x 17 samples over the cap's tangent square plus 5 % and 1 m. This is a measurement aid for what a min/max pyramid of the baked layer would give, not a guaranteed bound.
- **Plans:** `plan_terrain_field` at production parameters (`LodParams::earth_1080p`: cells 32, tau 6, finest level 22, morph band 0.3), with the camera 200 m (pitch 12 degrees) and 2 km (pitch 20 degrees) above the ground, heading north, 1080p.
- **Sites** (SPIKE-81's derived sites, seed 1): Black Forest site 1 (-7.75, 10.25), Alps site 1 (13.5, -117.25), Himalaya site 2 (11.75, -111.25); and today's Alps site (11.5, -18.5) as a control, where `k` is about 1.
- **Level steps:** as SPIKE-66. For the tile of each level 10-22 at the site and its 8 neighbours: the field at 65 x 65 child points against the bilinear surface of the tile's 33 x 33 vertices (p99 and max, metres). Also the centre tile's bound width against its sampled range; every sampled point was checked inside the bound (0 misses for `today` and `steep`).
- **Levers:** tau 4.5, 8, 4, 3, 2; morph band 0.15 and 0.5; cells 64 (`raw/levers.log`, `raw/levers-tau-himalaya.log`); tight bounds (`raw/tight.log`).
- **Runs:** BelowNormal under the heavy lock; read only on `global16384`.

## Results
### Plans at production parameters (nodes, capped leaves, deepest level)
| Site | Altitude | Today | Steep, no hill band | Steep | Steep, tight bounds |
|---|---|---|---|---|---|
| Black Forest | 200 m | 929, 0, 18 | 986, 0, 18 | 1,001, 0, 18 | - |
| Black Forest | 2 km | 665, 0, 15 | 700, 0, 15 | 713, 0, 15 | - |
| Alps | 200 m | 1,039, 0, 18 | 1,277, 4, 18 | 968, **85**, **15** | 1,048, 0, 18 |
| Alps | 2 km | 758, 0, 15 | 898, 3, 15 | 897, 28, 15 | 787, 0, 15 |
| Himalaya | 200 m | 1,022, 0, 18 | 626, **60**, **13** | 625, **62**, **13** | 1,036, 0, 18 |
| Himalaya | 2 km | 742, 0, 15 | 632, 56, 13 | 635, 62, 13 | 774, 0, 15 |
| Alps today (control) | 200 m | 1,003, 0, 18 | 1,039, 0, 18 | 1,045, 0, 18 | - |
| Alps today (control) | 2 km | 717, 0, 15 | 747, 0, 15 | 747, 0, 15 | - |
- **With the production bounds, the steepest classes degrade:** the Himalaya plans to level 13 with 56-62 capped leaves at both altitudes; the Alps with the hill band to level 15 (85 capped at 200 m). A level-13 tile has vertices about 38 m apart, against about 1 m at level 18.
- **The hills and the control do not degrade:** Black Forest and today's Alps site plan as today (0 capped, level 18), with up to 6 % more nodes.
- **With near-exact bounds nothing is capped:** the Himalaya and the Alps plan to level 18 with 0 capped and 1-4 % more nodes than today.
- Plan time: 0.2 s for the steep field against 0.1 s today (the tight variant's 2.5-4 s is its sampling, not a production cost).

### Level steps (tile against parent, p99 / max in metres) and bound width against the sampled range
| Level | Himalaya today | Himalaya steep | Alps steep | Black Forest steep |
|---|---|---|---|---|
| 10 | 6.70 / 14.4 (4.8x) | 46.9 / 92.8 (11x) | 34.0 / 78.1 (20x) | 11.7 / 27.2 (9.1x) |
| 12 | 2.06 / 5.22 (15x) | 13.3 / 33.9 (21x) | 10.6 / 23.3 (27x) | 3.42 / 7.32 (11x) |
| 14 | 0.62 / 1.47 (29x) | 4.04 / 9.51 (53x) | 3.19 / 6.58 (17x) | 1.00 / 2.44 (12x) |
| 16 | 0.16 / 0.33 (5.9x) | 1.06 / 2.15 (25x) | 0.84 / 2.02 (20x) | 0.29 / 0.62 (18x) |
| 18 | 0.03 / 0.09 (3.5x) | 0.22 / 0.58 (44x) | 0.16 / 0.44 (21x) | 0.06 / 0.15 (45x) |
| 20 | 0.003 / 0.02 (3.2x) | 0.02 / 0.14 (104x) | 0.005 / 0.10 (100x) | 0.007 / 0.03 (99x) |
- **The steps scale with `k`:** 5.5-7 times today's at the Himalaya and Alps (as SPIKE-66 found for a factor), about 5 times at the Black Forest. They are what a morph hides; a capped tile does not morph past its level.
- **The bounds are 5-50 times the sampled range at levels 12-18** for both fields (today's 3.5-29 times). With the steep field's ranges, that looseness eats the morph room.

### The levers (Himalaya and Alps, steep)
| Lever | Himalaya steep, 200 m | Alps steep, 200 m | Cost |
|---|---|---|---|
| Production (tau 6) | 625 nodes, 62 capped, level 13 | 968, 85, 15 | - |
| tau 4.5 | 1,113, 110, 14 | 2,094, 0, 19 | about 1.6x the nodes (today 1,644 at tau 4.5) |
| tau 3 | 3,287, 291, 17 | - | about 3.2x |
| tau 2 | 9,295, 0, 20 | - | about 9x the nodes |
| tau 8 | 359, 31, 12 | 421, 38, 12 | worse |
| Morph band 0.15 or 0.5 | unchanged | unchanged | no effect |
| Cells 64 | 95, 16, 8 | 100, 18, 8 | much worse |
| **Near-exact bounds** | **1,036, 0, 18** | **1,048, 0, 18** | a min/max pyramid of the baked layer (below) |
- Morph room needs `split + 2 rho` under the parent's split distance, with `rho` the neighbours' bounding radius from their height bounds (`morph_interval_for`). A finer tau widens the split distance; tighter bounds shrink `rho`. Only the bounds fix it at today's node count.

## Findings
1. **The steepest classes would degrade at production parameters with today's style of bounds:**
   - the Himalaya plans only to level 13 (60 capped leaves) and the Alps with the hill band to level 15 (85 capped);
   - the hills (Black Forest) and the control site do not degrade.
   - This would fail criterion 6 (no degradation) at the mountain classes.
2. **The cause is bound looseness, not the steepness itself.** With near-exact bounds the same terrain plans to level 18 with no capped tile at today's node count (plus 1-4 %).
   - The production bounds are 5-50 times the true range at levels 12-18, and the steep field's ranges are 5-7 times today's; the product leaves no morph room.
3. **The lever: per-tile min/max bounds from the bake.**
   - ADR 0088's erosion layer is baked anyway; storing a min/max pyramid of it per block (for example per 32 x 32 block, about 0.2-0.3 % of the layer with its coarser levels) gives exact bounds down to the bake's resolution.
   - Analytic bounds are needed only for the runtime detail below it (small).
   - It belongs in R4's layer format (ADR 0045's storage amendment) and in ADR 0016/0074's bounds contract.
4. **Tau is a poor lever:** tau 4.5 fixes the Alps at 1.6 times the nodes but not the Himalaya, and the Himalaya needs tau 2 (about 9 times the nodes). The morph band has no effect, and 64 cells per tile is worse.
5. **Level steps grow 5-7 times at the mountain classes** (Himalaya p99 at level 14: 0.62 to 4.0 m). Morphing hides them only where morph room exists, so finding 3 also covers the steps.
6. **ADR re-check verdicts** (for SPIKE-76's table):
   - ADR 0014 (LOD algorithm): keep;
   - ADR 0016 (height-aware selection): keep, amended to read baked min/max bounds where a layer provides them;
   - ADR 0074 (capping): keep; with exact bounds it rarely acts, and it is the right safety net.

## Open points
- The steep field is a stand-in: today's eroded field scaled by `k` with a delta and a hill band, not ADR 0088's bake (the carve and thermal would lower the slope tail, SPIKE-74).
- The hill-band bound assumes the warp is at most 5.5 times Lipschitz on a cap; no sampled point missed it, but it is not proven.
- The tight variant's bounds are sampled, not guaranteed (one level-12 check at the Alps found a sample outside them). A real min/max pyramid is exact at the bake's cells and needs the detail's analytic bound below them.
- Two altitudes, four poses, one seed; no GPU frame time (that is a perf window, criterion 6).
- Held-back tiles in the engine (streaming) were not measured; this is the planner alone.

## Handover
- On branch `spike/m9-plains`: `bash proto/run.sh.txt` (plans and steps at the four sites; needs `target/s81/s81-fields.f32` from SPIKE-81's `classdrivers.py`), `bash proto/levers.sh.txt` (tau, morph band, cells). The test takes `S82_TAU`, `S82_MORPH` and `S82_CELLS`.
