# SPIKE-78: belt height and the badlands class

Statistics produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. Citation: https://doi.org/10.5270/ESA-c5d3d65

Opened: 2026-10-31
Question:
- **Belts:** land above 2 km is 5 % in the generator (SPIKE-77). What limits the belts' height (base elevation rule, clamp, bake)? Does one change at coarse global resolution bring the shares above 2 and 4 km to Earth's, and what does it do to the Alps and Himalaya sites under the class pipeline?
- **Badlands site 1:** is an authored class with no carve and `k` about 1 "not worse" than the baseline?
Why it matters: the relief-class proposal (SPIKE-76, 77) needs high ground for its mountain classes; the arid class needs a rule that does not add relief.
Time box: 2.5 h (opus-planner, 2026-10-31); used about 45 min, with 5 min of machine time.
Success criteria:
- the limiter named, with numbers;
- one change scored against Earth's shares above 2 and 4 km;
- the Alps and Himalaya sites scored with the change;
- the Badlands verdict.
Code location: the global analysis is a Python prototype (`proto/belts.py.txt`) on SPIKE-69's 0.5-degree map of the generator's inputs (`target/s69/map13.f32`). The sites run on branch `spike/m9-plains` (commit 37606e72; `S78_DIR` in the `terrain_score` example, `proto/s78_block.rs.txt`, `proto/run.sh.txt`). Spike code, never merged.
Closed: 2026-10-31

## Method
- **Today's rule** (`global.rs` `sample`): elevation = `base + 5200 ridge zone inland + hill band`, clamped to +-9,000 m.
  - `base` rises to 1,000 m with the distance from the coast;
  - `ridge` = `max(1 - 2|fBm|, 0)^2` (5 octaves);
  - `zone` = smoothstep of a range-mask fBm; `inland` = smoothstep(0.02, 0.35) of the land depth.
- **Global shares:** on land under 60 degrees latitude, area-weighted, 720 x 360 map. The belt term is rebuilt from the map's zone, inland and ridge; variants change only the belt term. SPIKE-77's plateau rule (window 0.50-0.62, 1,200 m) is added for the combined shares.
- **Earth:** Steve Dutch's ETOPO5 5-minute hypsometry tables (stevedutch.net, "Earth hypsometry, 5 minute"), sums of the continent tables without Antarctica. Land under 60 degrees has no Antarctica, so these are the fair targets; the shares with the ice sheets are listed for reference.
  | Land above | 1 km | 2 km | 3 km | 4 km | 5 km |
  |---|---|---|---|---|---|
  | Earth without Antarctica | about 20 % (SPIKE-77's estimate) | 6.7 % | 3.2 % | 1.9 % | 0.8 % |
  | Earth with ice | 27.2 % | 11.7 % | 5.1 % | 1.8 % | 0.7 % |
- **Variants:** amplitude 7,000 or 8,000 m; ridge to the power 1 instead of 2; inland as a coast gate (`smoothstep(0, 0.3)`) instead of a weight; a broad range uplift `U zone inland` without the ridge; and a grid of amplitude 4,000-8,000 m by uplift 0-3,000 m, both core modes. Score: the sum of `|log(generator / Earth)|` over 2, 3, 4 and 5 km.
- **Sites:** SPIKE-76's class pipeline (`high mountains`, `k` 7.5, no floor) at the Alps and Himalaya sites, with and without the belt change. The change enters as a height delta `(7000 - 5200) ridge zone inland` on the fine and coarse windows before the pipeline. This stands in for a re-baked store: the delta has wavelengths over 100 km. The sites stay where SPIKE-55 matched them; a re-match is not done.
- **Badlands:** `class` (SPIKE-76: `k` 1, both carve passes), `nocarve` (`k` 1, no floor, no carve, thermal and deposition kept) and `nocarve-bare` (`k` 1, no pass at all).
- **Runs:** BelowNormal under the heavy lock (`raw/run.log`, `raw/belts.log`).

## Results
### What limits the belts
| Factor (land under 60 degrees) | Share above 0.5 |
|---|---|
| `zone` | 21.2 % |
| `inland` | 65.5 % |
| `ridge` | 46.6 % |
| `zone inland` | 6.2 % |
| `inland` where `zone` is above 0.5 | 37.3 % |
- **The rule, not the bake or the clamp, limits the belts.**
  - The belt term is a product of three factors in `[0, 1]`, and one of them is squared. Its p99 on land is 2,790 m, its maximum 5,175 m of 5,200.
  - In this seed `zone` and `inland` rarely peak together: range zones lie near the coasts more often than land does.
  - The highest land is 6,333 m: the 9,000 m clamp never acts.
  - The store's field follows the rule: field minus sample is p50 -20 m, -56 m where the sample is above 3 km (`raw/field-vs-sample.log`); shares above 2 / 4 km 4.2 / 0.7 % against the sample's 4.3 / 0.7 %.

### Global shares (land above, under 60 degrees)
| Variant | 1 km | 2 km | 3 km | 4 km | 5 km | Log error | Highest |
|---|---|---|---|---|---|---|---|
| Today | 12.1 % | 4.3 % | 1.6 % | 0.7 % | 0.2 % | 3.37 | 6,333 m |
| Today + plateau | 25.6 % | 5.1 % | 1.7 % | 0.7 % | 0.2 % | 3.12 | 6,333 m |
| **Amplitude 7,000 + plateau** | **27.7 %** | **7.1 %** | **3.2 %** | **1.4 %** | **0.75 %** | **0.42** | 8,125 m |
| Amplitude 8,000 + plateau | 28.7 % | 8.0 % | 4.0 % | 1.9 % | 1.07 % | 0.77 | 9,000 m (clamped) |
| Ridge^1 + plateau | 29.1 % | 7.9 % | 3.0 % | 1.2 % | 0.48 % | 1.16 | 6,346 m |
| Inland as a coast gate + plateau | 29.4 % | 7.0 % | 2.6 % | 1.1 % | 0.28 % | 1.82 | 6,333 m |
| Range uplift 1,500 + plateau | 30.4 % | 9.1 % | 4.0 % | 1.7 % | 0.91 % | 0.86 | 7,833 m |
| Earth without Antarctica | about 20 % | 6.7 % | 3.2 % | 1.9 % | 0.8 % | - | 8,849 m |
- The grid's best is amplitude 7,000 m without the broad uplift (`raw/belts.json`, the eight best). Uplift and a lower amplitude trade off (6,000 m + 750 m uplift scores 0.56).
- **Widening (ridge^1, coast gate) fills 2 km but not 4-5 km.** Raising the amplitude moves the whole tail.
- The highest land at 7,000 m is 8,125 m: under the clamp with room for the hill band (7,000 + 1,000 base + 1,000 band = 9,000 m).
- `images/elevation-today-vs-belts.png`: elevation 0 to 6 km, today (left) and amplitude 7,000 m with the plateau rule (right).
- The high ground stays in three compact spots, not long belts: the range mask is a blob-shaped fBm. That is the shape of the belts, a separate item from their height.

### Alps and Himalaya under the class pipeline (generator over real; distances in brackets)
| Site | Variant | Base elevation (belt delta p50) | Slope | Relief 1 km | Relief 5 km | Classes L1 |
|---|---|---|---|---|---|---|
| Alps 1 | baseline | 1,374 m | 0.12 (0.43) | 0.11 (0.46) | 0.16 (0.57) | 0.26 |
| Alps 1 | class | | 1.11 (0.08) | 0.75 (0.15) | 0.82 (0.17) | 0.28 |
| Alps 1 | **class + belt** | (+426 m) | 1.20 (0.10) | **0.87 (0.09)** | **0.99 (0.08)** | 0.21 |
| Alps 2 | baseline | 1,327 m | 0.11 (0.44) | 0.10 (0.46) | 0.15 (0.59) | 0.22 |
| Alps 2 | class | | 1.02 (0.08) | 0.61 (0.21) | 0.64 (0.26) | 0.37 |
| Alps 2 | **class + belt** | (+394 m) | 1.07 (0.08) | **0.66 (0.19)** | **0.70 (0.22)** | 0.33 |
| Himalaya 1 | baseline | 3,221 m | 0.23 (0.63) | 0.24 (0.78) | 0.30 (0.86) | 0.09 |
| Himalaya 1 | class | | 1.19 (0.17) | 1.07 (0.20) | 1.19 (0.19) | 0.26 |
| Himalaya 1 | class + belt | (+882 m) | 1.21 (0.18) | 1.11 (0.23) | 1.22 (0.24) | 0.24 |
| Himalaya 2 | baseline | 2,122 m | 0.14 (0.71) | 0.14 (0.89) | 0.17 (1.01) | 0.07 |
| Himalaya 2 | class | | 0.99 (0.07) | 0.70 (0.27) | 0.71 (0.36) | 0.34 |
| Himalaya 2 | **class + belt** | (+668 m) | 1.02 (0.05) | **0.74 (0.23)** | **0.76 (0.29)** | 0.30 |
Real: Alps slope 22.3 degrees, relief 1 / 5 km 391 / 1,039 m; Himalaya 29.1 degrees, 524 / 1,573 m.
- Criterion 1 (slope and 1 km relief at 0.6 of real or more at both sites): the Alps pass with the class alone (0.61 at site 2) and with a wider margin with the belts (0.66).
- The belts mostly lift the 5 km relief: Alps 0.82 to 0.99 and 0.64 to 0.70. Every distance falls except Himalaya site 1, where the class already overshoots (1.07-1.22) and the belts add a little more.

### Badlands with no carve (generator over real; distances in brackets)
| Site | Variant | Slope | Relief 1 km | Relief 5 km distance | Flat (real 2.6 %) | Classes L1 |
|---|---|---|---|---|---|---|
| 1 | baseline | 0.83 (0.12) | 0.87 (0.11) | 0.04 | 0.0 % | 0.31 |
| 1 | class (`k` 1, carve) | 0.96 (0.06) | 0.63 (0.22) | 0.53 | 0.1 % | 0.43 |
| 1 | **nocarve** (`k` 1) | 0.83 (0.12) | 0.87 (0.11) | 0.04 | 0.0 % | 0.31 |
| 2 | baseline | 1.39 (0.10) | 1.36 (0.21) | 0.36 | 0.0 % | 0.23 |
| 2 | class (`k` 1, carve) | 2.43 (0.48) | 1.83 (0.58) | 0.78 | 0.0 % | 0.57 |
| 2 | **nocarve** (`k` 1) | 1.39 (0.10) | 1.36 (0.21) | 0.36 | 0.0 % | 0.23 |
- `nocarve` and `nocarve-bare` give the same numbers as the baseline at both sites: at slopes of 2.5-4 degrees, thermal at 33 degrees and deposition without a carve do nothing.
- **Verdict: not worse** (equal on every distance). The carve alone made the `k` 1 class worse (SPIKE-76).

## Findings
1. **The belts are limited by their rule, not by the bake or the clamp.**
   - `5200 ridge^2 zone inland` multiplies three factors in `[0, 1]`; `zone` and `inland` are both above 0.5 on only 6 % of land, and the highest land is 6.3 km.
   - The store's field adds nothing and removes about 20-60 m.
2. **One constant fixes the shares:** amplitude 5,200 to 7,000 m with SPIKE-77's plateau rule gives 7.1 / 3.2 / 1.4 / 0.75 % above 2 / 3 / 4 / 5 km against Earth's 6.7 / 3.2 / 1.9 / 0.8 % (without Antarctica).
   - It needs no new term and keeps the clamp (highest 8,125 m).
   - Widening the belts fills 2 km but not the top; a broad range uplift does no better than the amplitude.
   - The planner's 12 % above 2 km is Earth with the Antarctic and Greenland ice; land under 60 degrees compares with 6.7 %.
3. **Higher belts help the Alps' margin under the class pipeline:** 1 km relief 0.75 / 0.61 to 0.87 / 0.66, 5 km relief 0.82 / 0.64 to 0.99 / 0.70. The Himalaya site 2 gains (0.70 to 0.74); site 1 already overshoots.
4. **An authored badlands class with no carve and `k` 1 is "not worse" at both sites:** it leaves the field as it is (equal distances). Site 1 is close to real today; site 2 keeps its baseline gap (1 km relief 1.36).
5. Both changes are generator-rule changes: the belt amplitude changes the global bake's base elevation (GLOBAL_VERSION bump and a store re-bake, as the plateau rule already does).

## Open points
- **Belt shape:** the high ground forms compact spots, not long belts (the range mask is blob-shaped fBm). Not scored here.
- **The site delta is a stand-in:** the belt change was added to the stored field, not re-baked; the bake's own detail and erosion field over the higher base are not included. The sites were not re-matched to the new elevation.
- **Class shares with higher belts** were not recomputed: SPIKE-77's classes use the field's 7 km relief, which the belts raise in the range cores (mountain cells already).
- One seed; Earth's 1 km share without Antarctica is SPIKE-77's estimate.

## Visuals
- `images/elevation-today-vs-belts.png`: elevation 0 to 6 km, equirectangular, 0.5 degree; today (left), amplitude 7,000 m with the plateau rule (right).

## Handover
- **Global shares:** `python -I proto/belts.py.txt <SPIKE-69 map13.f32> <out>` (about 1 min).
- **Sites:** on branch `spike/m9-plains`, build the `terrain_score` example and run `proto/run.sh.txt` (`S78_AMP`, default 7000; `S78_BK`, the Badlands `k`, default 1).
