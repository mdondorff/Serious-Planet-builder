# SPIKE-73: why the plains are 91 % flat

Statistics produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. Citation: https://doi.org/10.5270/ESA-c5d3d65

Opened: 2026-10-31
Question: Why does the scorecard (TEST-079) class the generator's plains as 90-91 % flat (Brandenburg; real 23 %), and Loess about 50 % flat (real 1 %)? What fixes it?
- G1's fine octaves;
- the hybrid bake's local carve;
- a plains relief floor in `k`;
- or is the class's flatness threshold an artefact?
Why it matters: the scorecard baseline's largest gap on the plains (`docs/milestones/M9/terrain-score-baseline.md`); ADR 0088's `k` is 1 on about half the land.
Time box: 2.5 h (opus-planner, 2026-10-31); used about 1 h 30 min, with 15 min of machine time.
Success criteria: the cause; each candidate scored with the scorecard's own descriptors at the three plains landforms (Brandenburg, Loess, Erg Chebbi, two sites each).
Code location: branch `spike/m9-plains` (pushed, no PR; commit 23df05ce; spike code, never merged):
- the `terrain_score` example takes `S73_DIR` and scores variants of the plains rasters (`proto/spike_block.rs.txt`);
- `erosion.rs` takes `SPIKE73_FINE_FLOOR`.
The 30 m local carve is an offline crate made from SPIKE-72's (`proto/make_carve.py.txt`).
Closed: 2026-10-31

## Method
- **Rasters:** the scorecard's own: the central 0.5-degree square of each GLO-30 tile at 30 m, and the generator's two matched sites at 30 m (`world.field()` of `global16384`, read only). The descriptors are the scorecard's (GEN-089 to GEN-092).
- **Flat:** the geomorphon class (GEN-091): no line out to 1 km in any of 8 directions rises or falls more than 1 degree.
- **Printed per raster** (`raw/`): the flat share, the three largest classes, slope p50, 1 km relief p50; against the real tile, the class L1 (0 to 2) and the relief-1 km distance.
- **Variants:**
  | Variant | What |
  |---|---|
  | real box 3x3, 5x5 | the real tile smoothed (GLO-30 is a surface model: trees and buildings) |
  | k 1.5 to 3 | the generator `m + k (h - m)`, `m` the 10 km box mean: a plains relief floor in `k` (ADR 0088's formula) |
  | noise 0.1, 0.3, 1 m | white noise per 30 m cell: G1's sub-30 m octaves as the scorecard samples them (law a is about 0.1 m at a plains share of 0.3-0.4) |
  | carve, carve-nouplift | SPIKE-72's incising local carve (no fill, no breach, area cap 1 km^2) on the 30 m raster, with uplift (U/K 20) and without |
  | fine floor 0.25, 0.5, 1 | the fine creases' factor `carve + (1 - carve) L` (GEN-061) held at least at the floor: fine relief on plains without touching the 7 km scale |
- **Global check:** SPIKE-69's 0.5-degree map, the class shares with a `k` floor on all land.

## Results
### The cause
- The generator's plains lack relief between 30 m and 1 km: slope p50 0.56 against 1.15 degrees and 1 km relief 8-9 m against 23 m (Brandenburg). Under 17 m over 1 km is under the 1-degree threshold everywhere, hence "flat".
- In the field, the crease octaves under 1.6 km are scaled by `carve + (1 - carve) L`. On a flat bake (`carve` small) with `L` 0 that is about 0: the plains get no fine creases (GEN-061).
- **The threshold is partly an artefact of the real side:** smoothing the real tile 3x3 or 5x5 raises its flat share from 22.6 % to 30.8-37.4 % (Brandenburg) and from 48 % to 56-63 % (Erg Chebbi). Surface clutter accounts for about a third of the real roughness at Brandenburg, not the gap.

### Candidates (flat share; class L1 / relief-1 km distance against real)
| Variant | Brandenburg (real 23 % flat) | Loess (real 1 %) | Erg Chebbi (real 48 %) |
|---|---|---|---|
| base | 90-91 %; 1.38-1.39 / 0.35-0.36 | 49-52 %; 1.43-1.48 / 0.76-0.78 | 0 %; 1.28-1.30 / 0.26-0.37 |
| k 2 | 23 %; 0.69-0.74 / 0.17-0.18 | 1 %; 0.65-0.66 / 0.34-0.39 | 0 %; 1.30 / 0.64-0.91 |
| k 2.5 | 7 %; 0.85-0.91 / 0.09-0.11 | 0 %; 0.57-0.60 / 0.14-0.19 | 0 %; worse |
| noise 0.1 m (G1 law a) | 86-88 %; 1.34-1.36 | 45-48 %; 1.40-1.46 | unchanged |
| noise 1 m | 0 %; 0.64 (ridge and valley noise) | 0 %; 0.31-0.32 | unchanged |
| carve, no uplift | 33 %; 0.66 / 0.31 | 9-11 %; 0.72-0.74 / 0.65-0.68 | 0 %; 1.33-1.34 |
| carve with uplift | 0 %: 70 % peaks and pits (an artefact, below) | the same | the same |
| fine floor 0.25 | 64-68 %; 1.05-1.10 / 0.31-0.33 | 10-11 %; 0.95-0.96 / 0.66-0.67 | unchanged |
| fine floor 0.5 | 5-6 %; 0.74-0.77 / 0.20 | 0 %; **0.36** / 0.35-0.36 | unchanged |
| fine floor 1 | 0 %; 0.84 / 0.20-0.21 | 0 %; **0.31** / 0.34-0.36 | 0 %; 1.30 / 0.50-0.53 |

### The global check (SPIKE-69's map, classes by 7 km-radius relief)
| `k` floor on all land | Plains / hills / mountains |
|---|---|
| 1 (SPIKE-69) | 0.626 / 0.124 / 0.250 |
| 1.5 | 0.481 / 0.270 / 0.250 |
| 2 | 0.351 / 0.399 / 0.250 |
- A floor of 2, enough for Brandenburg's flat share, moves a quarter of the land from plains to hills (Earth 61 % plains).

## Findings
1. **The plains are flat because the fine creases are switched off on them.** Their factor `carve + (1 - carve) L` is about 0 on a flat bake with `L` 0, so there is no relief between 30 m and 1 km. The 1-degree class threshold then reads them as flat. The real side's surface clutter is a smaller part (smoothing moves real Brandenburg from 23 % to 31-37 % flat).
2. **A fine-crease floor is the targeted fix:**
   - about 0.35 for Brandenburg (between 0.25: 64-68 % flat and 0.5: 5-6 %);
   - 0.5 to 1 for Loess: class L1 from 1.43-1.48 to 0.31-0.36, the best of all variants;
   - it acts below 1.6 km only, so SPIKE-69's 7 km classes are not touched (not re-measured here).
3. **A `k` floor matches Brandenburg's flat share at k 2 but breaks the global shares** (plains 61 % to 35 %): `k` scales everything under 20 km, and the plains need fine relief, not more 7 km relief.
4. **G1 (law a) is too small to matter here:** about 0.1 m at 30 m sampling moves the flat share by 3-4 points. At 1 m it only fakes ridge and valley classes.
5. **The 30 m local carve without uplift** halves the flats (Brandenburg 33 %, Loess 9-11 %) but adds little 1 km relief (10-17 m against 23-38 m).
6. **Warning for SPIKE-72's fix:** with uplift, a carve that only incises raises draining cells by up to 20 m while pits stay, giving 70 % peaks and pits at 30 m. The local pass of ADR 0088 should run without uplift. SPIKE-72's higher slope (+16-36 %) is probably partly this; re-measure in R4 PR 5.
7. **Erg Chebbi goes the other way:** the generator is too rough there (0 % flat against 48 %, 1 km relief 33-43 m against 12 m). No candidate helps; it would need `k` under 1 or other matched sites. ADR 0088's `k` is at least 1.

## Recommendation
- **Generator (R7 or an M9 generator change):** a floor on the fine creases' factor, starting at about 0.4, behind the ADR 0088 switch. Score it with `cargo xtask terrain-score`. It is the targeted fix for Brandenburg and Loess, cheap (no bake), and leaves the 7 km classes.
- **ADR 0088 / m9-bake:** pass 2's local carve without uplift (finding 6).
- **Desert:** the Erg Chebbi sites need a decision: `k` below 1 for a desert landform, or re-matched sites (SPIKE-55's matching by 1-degree statistics picks rough ground).

## Open points
- The fine floor's effect on SPIKE-69's 7 km class shares and on the hill sites was not re-measured (it raises fine relief everywhere `L` and `carve` are low).
- The floor value between 0.25 and 0.5 for Brandenburg was not bisected.
- The real side's clutter: the scorecard could score a smoothed real tile as well; a scorecard design question (TEST-079), not changed here.

## Visuals
- None: the measures are tables of the scorecard's descriptors (`raw/`). The scorecard's `--maps` gives the class maps per raster if needed.

## Handover
- **Variants:** on branch `spike/m9-plains`, build the `terrain_score` example and run it with `S73_DIR=<dir>` (`proto/run1.sh.txt`); add `SPIKE73_FINE_FLOOR=<f>` for the floor (`proto/run3.sh.txt`).
- **The 30 m carve:** `python -I proto/make_carve.py.txt` builds the crate from SPIKE-72's. Run it on the rasters `S73_DIR` writes (`proto/run2.sh.txt`); a second `S73_DIR` run scores them.
