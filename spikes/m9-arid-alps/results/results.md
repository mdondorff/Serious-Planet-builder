# SPIKE-75: arid landforms and the Alps under the pipeline

Statistics produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. Citation: https://doi.org/10.5270/ESA-c5d3d65

Opened: 2026-10-31
Question: Why do the Badlands and Erg Chebbi get a high `k` under SPIKE-74's pipeline? Does a rule that holds arid landforms at `k` of 1 or below fix them? What top knot do the Alps need? Does a crease floor applied only where `k` is about 1 stop the plains' overshoot? And (inbox #17): how do the field's elevation ramps interact with `k`, would an elevation term in `k` help, and is an elevation-gated amplitude sound?
Why it matters: R7 pre-tuning; criterion 1's "not worse" clause failed in SPIKE-74.
Time box: 2.5 h (opus-planner, 2026-10-31); used about 1 h 20 min, with 25 min of machine time (after waiting 25 min for a perf window's lock).
Success criteria: `k`, L, the base elevation and the climate logged at all 18 sites; the variants scored like SPIKE-74; verdicts per landform.
Code location: branch `spike/m9-plains` (spike code, never merged):
- the `terrain_score` example takes `S75_DIR` (`proto/s75_block.rs.txt`);
- `global::spike69_parts` is ported to the branch.
Closed: 2026-10-31

## Method
- **Pipeline:** SPIKE-74's (fine-crease floor 0.35; `k`; pass 1 at 814 m; pass 2 incise only without uplift; active thermal; deposition) on the scorecard's 30 m rasters.
- **Variants:**
  | Variant | `k` and floor |
  |---|---|
  | ref | SPIKE-74 as run (reproduces its numbers) |
  | arid | `k` held toward 1 by the generator's precipitation: `1 + (k - 1) smoothstep(250, 500 mm, P)`; the floor only where `k` is about 1 (weight `1 - smoothstep(1, 1.5, k)`) |
  | arid+top8 | as arid, the top knot at L 1 raised from 5.6 to 8 |
  | sand | as arid+top8, and `k` down toward 0.4 under 150 mm |
- **Site log:** the generator's values at each site's centre (`raw/run.log`, lines `S75SITE`).
- **Runs:** BelowNormal under the heavy lock.

## Results
### The sites as the generator sees them
| Landform | Site | L | `k` (ref) | Bake elevation | Precipitation | Zone / inland / ridge |
|---|---|---|---|---|---|---|
| Brandenburg | 1, 2 | 0.010, 0.026 | 1.14, 1.29 | 84, 74 m | 977, 1,381 mm | 0.15/0.07/0.91, 1.00/0.02/0.43 |
| Loess | 1, 2 | 0, 0 | 1.00, 1.00 | 405, 396 m | 608, 1,087 mm | 0.00/1.00/0.71, 0.00/1.00/0.53 |
| Badlands | 1, 2 | 0.397, 0.854 | 4.80, 5.42 | 812, 876 m | 1,011, 410 mm | 0.68/0.58/0.30, 0.85/1.00/0.10 |
| Erg Chebbi | 1, 2 | 0.359, 0.550 | 4.75, 5.01 | 989, 954 m | 179, 1,274 mm | 0.67/0.54/0.42, 0.55/1.00/0.18 |
| Valley and Ridge | 1, 2 | 0.143, 0.139 | 4.32, 4.14 | 345, 320 m | 935, 1,090 mm | 0.58/0.25/0.36, 0.76/0.18/0.34 |
| Black Forest | 1, 2 | 0.177, 0.104 | 4.50, 4.50 | 516, 589 m | 2,078, 762 mm | 0.36/0.49/0.41, 0.41/0.25/0.95 |
| Highlands | 1, 2 | 0.096, 0.072 | 3.93, 4.47 | 297, 378 m | 1,457, 100 mm | 0.61/0.15/0.48, 0.66/0.11/0.89 |
| Alps | 1, 2 | 0.247, 0.302 | 4.60, 4.67 | 1,374, 1,327 m | 1,300, 893 mm | 0.70/0.35/0.96, 0.84/0.36/0.73 |
| Himalaya | 1, 2 | 0.649, 0.398 | 5.15, 4.80 | 3,221, 2,122 m | 834, 1,689 mm | 0.90/0.72/0.75, 0.98/0.41/0.93 |

### Scores (generator over real; distances in brackets)
### Brandenburg plain (N52E012)
Real: slope p50 1.15 deg, relief 1 km p50 22.9 m, flat 22.6 %.

| Site, variant | Slope (gen/real; distance) | Relief 1 km (gen/real; distance) | Relief 5 km distance | Flat | Classes L1 |
|---|---|---|---|---|---|
| 1 baseline | 0.49 (0.19) | 0.38 (0.35) | 0.39 | 91.0 % | 1.39 |
| 1 ref | 2.01 (0.09) | 1.06 (0.09) | 0.26 | 1.2 % | 0.81 |
| 1 arid | 1.96 (0.09) | 1.05 (0.08) | 0.25 | 1.4 % | 0.82 |
| 1 arid+top8 | 1.96 (0.09) | 1.05 (0.08) | 0.25 | 1.4 % | 0.82 |
| 1 sand | 1.96 (0.09) | 1.05 (0.08) | 0.25 | 1.4 % | 0.82 |
| 2 baseline | 0.49 (0.19) | 0.36 (0.36) | 0.44 | 89.6 % | 1.38 |
| 2 ref | 1.99 (0.10) | 1.01 (0.06) | 0.13 | 1.5 % | 0.79 |
| 2 arid | 1.76 (0.09) | 0.94 (0.06) | 0.10 | 2.4 % | 0.81 |
| 2 arid+top8 | 1.76 (0.09) | 0.94 (0.06) | 0.10 | 2.4 % | 0.81 |
| 2 sand | 1.76 (0.09) | 0.94 (0.06) | 0.10 | 2.4 % | 0.81 |

### Loess hills and plains (N41W096)
Real: slope p50 3.88 deg, relief 1 km p50 38.2 m, flat 1.0 %.

| Site, variant | Slope (gen/real; distance) | Relief 1 km (gen/real; distance) | Relief 5 km distance | Flat | Classes L1 |
|---|---|---|---|---|---|
| 1 baseline | 0.22 (0.39) | 0.33 (0.78) | 0.64 | 52.4 % | 1.48 |
| 1 ref | 0.93 (0.18) | 0.97 (0.52) | 1.01 | 0.1 % | 0.62 |
| 1 arid | 0.93 (0.18) | 0.97 (0.52) | 1.01 | 0.1 % | 0.62 |
| 1 arid+top8 | 0.93 (0.18) | 0.97 (0.52) | 1.01 | 0.1 % | 0.62 |
| 1 sand | 0.93 (0.18) | 0.97 (0.52) | 1.01 | 0.1 % | 0.62 |
| 2 baseline | 0.23 (0.39) | 0.35 (0.76) | 0.56 | 48.9 % | 1.43 |
| 2 ref | 0.89 (0.10) | 0.93 (0.23) | 0.50 | 0.3 % | 0.63 |
| 2 arid | 0.89 (0.10) | 0.93 (0.23) | 0.50 | 0.3 % | 0.63 |
| 2 arid+top8 | 0.89 (0.10) | 0.93 (0.23) | 0.50 | 0.3 % | 0.63 |
| 2 sand | 0.89 (0.10) | 0.93 (0.23) | 0.50 | 0.3 % | 0.63 |

### Badlands (N43W102)
Real: slope p50 3.04 deg, relief 1 km p50 39.5 m, flat 2.6 %.

| Site, variant | Slope (gen/real; distance) | Relief 1 km (gen/real; distance) | Relief 5 km distance | Flat | Classes L1 |
|---|---|---|---|---|---|
| 1 baseline | 0.83 (0.12) | 0.87 (0.11) | 0.04 | 0.0 % | 0.31 |
| 1 ref | 4.31 (0.97) | 2.94 (1.24) | 1.77 | 0.0 % | 0.44 |
| 1 arid | 4.31 (0.97) | 2.94 (1.24) | 1.77 | 0.0 % | 0.44 |
| 1 arid+top8 | 4.84 (1.11) | 3.31 (1.47) | 2.14 | 0.0 % | 0.41 |
| 1 sand | 4.84 (1.11) | 3.31 (1.47) | 2.14 | 0.0 % | 0.41 |
| 2 baseline | 1.39 (0.10) | 1.36 (0.21) | 0.36 | 0.0 % | 0.23 |
| 2 ref | 7.65 (1.70) | 5.48 (2.78) | 3.93 | 0.0 % | 0.28 |
| 2 arid | 6.32 (1.42) | 4.38 (2.08) | 2.90 | 0.0 % | 0.34 |
| 2 arid+top8 | 7.69 (1.71) | 5.53 (2.80) | 3.98 | 0.0 % | 0.27 |
| 2 sand | 7.69 (1.71) | 5.53 (2.80) | 3.98 | 0.0 % | 0.27 |

### Erg Chebbi desert (N31W004)
Real: slope p50 0.86 deg, relief 1 km p50 11.8 m, flat 47.9 %.

| Site, variant | Slope (gen/real; distance) | Relief 1 km (gen/real; distance) | Relief 5 km distance | Flat | Classes L1 |
|---|---|---|---|---|---|
| 1 baseline | 2.75 (0.32) | 2.83 (0.26) | 0.29 | 0.0 % | 1.28 |
| 1 ref | 16.83 (2.95) | 11.42 (1.59) | 1.74 | 0.0 % | 1.31 |
| 1 arid | 6.23 (1.25) | 4.98 (0.64) | 0.68 | 0.0 % | 1.39 |
| 1 arid+top8 | 6.23 (1.25) | 4.98 (0.64) | 0.68 | 0.0 % | 1.39 |
| 1 sand | 6.23 (1.25) | 4.98 (0.64) | 0.68 | 0.0 % | 1.39 |
| 2 baseline | 3.67 (0.48) | 3.65 (0.37) | 0.38 | 0.0 % | 1.30 |
| 2 ref | 19.85 (3.36) | 13.25 (1.88) | 1.98 | 0.0 % | 1.29 |
| 2 arid | 19.85 (3.36) | 13.25 (1.88) | 1.98 | 0.0 % | 1.29 |
| 2 arid+top8 | 23.36 (3.85) | 15.85 (2.29) | 2.44 | 0.0 % | 1.29 |
| 2 sand | 23.36 (3.85) | 15.85 (2.29) | 2.44 | 0.0 % | 1.29 |

### Valley and Ridge (N40W078)
Real: slope p50 8.11 deg, relief 1 km p50 120.3 m, flat 0.0 %.

| Site, variant | Slope (gen/real; distance) | Relief 1 km (gen/real; distance) | Relief 5 km distance | Flat | Classes L1 |
|---|---|---|---|---|---|
| 1 baseline | 0.18 (0.36) | 0.17 (0.46) | 0.88 | 4.6 % | 0.52 |
| 1 ref | 1.16 (0.04) | 0.73 (0.21) | 0.46 | 0.0 % | 0.63 |
| 1 arid | 1.00 (0.03) | 0.67 (0.23) | 0.49 | 0.0 % | 0.65 |
| 1 arid+top8 | 1.00 (0.03) | 0.67 (0.23) | 0.49 | 0.0 % | 0.65 |
| 1 sand | 1.00 (0.03) | 0.67 (0.23) | 0.49 | 0.0 % | 0.65 |
| 2 baseline | 0.17 (0.36) | 0.16 (0.46) | 0.88 | 6.1 % | 0.57 |
| 2 ref | 1.23 (0.08) | 0.80 (0.17) | 0.33 | 0.0 % | 0.62 |
| 2 arid | 1.07 (0.03) | 0.73 (0.19) | 0.37 | 0.0 % | 0.64 |
| 2 arid+top8 | 1.07 (0.03) | 0.73 (0.19) | 0.37 | 0.0 % | 0.64 |
| 2 sand | 1.07 (0.03) | 0.73 (0.19) | 0.37 | 0.0 % | 0.64 |

### Black Forest (N48E008)
Real: slope p50 9.06 deg, relief 1 km p50 140.6 m, flat 0.0 %.

| Site, variant | Slope (gen/real; distance) | Relief 1 km (gen/real; distance) | Relief 5 km distance | Flat | Classes L1 |
|---|---|---|---|---|---|
| 1 baseline | 0.18 (0.35) | 0.17 (0.46) | 0.54 | 1.8 % | 0.58 |
| 1 ref | 1.32 (0.07) | 0.83 (0.17) | 0.17 | 0.0 % | 0.44 |
| 1 arid | 1.23 (0.06) | 0.80 (0.18) | 0.18 | 0.0 % | 0.46 |
| 1 arid+top8 | 1.24 (0.06) | 0.80 (0.17) | 0.17 | 0.0 % | 0.45 |
| 1 sand | 1.24 (0.06) | 0.80 (0.17) | 0.17 | 0.0 % | 0.45 |
| 2 baseline | 0.16 (0.36) | 0.16 (0.47) | 0.54 | 6.1 % | 0.84 |
| 2 ref | 1.37 (0.07) | 0.88 (0.16) | 0.15 | 0.0 % | 0.43 |
| 2 arid | 1.15 (0.06) | 0.79 (0.17) | 0.15 | 0.0 % | 0.46 |
| 2 arid+top8 | 1.15 (0.06) | 0.79 (0.17) | 0.15 | 0.0 % | 0.46 |
| 2 sand | 1.15 (0.06) | 0.79 (0.17) | 0.15 | 0.0 % | 0.46 |

### Scottish Highlands (N57W005)
Real: slope p50 6.58 deg, relief 1 km p50 127.1 m, flat 4.9 %.

| Site, variant | Slope (gen/real; distance) | Relief 1 km (gen/real; distance) | Relief 5 km distance | Flat | Classes L1 |
|---|---|---|---|---|---|
| 1 baseline | 0.18 (0.30) | 0.14 (0.39) | 0.58 | 16.3 % | 0.67 |
| 1 ref | 1.37 (0.11) | 0.70 (0.15) | 0.29 | 0.0 % | 0.75 |
| 1 arid | 1.12 (0.05) | 0.62 (0.18) | 0.31 | 0.0 % | 0.78 |
| 1 arid+top8 | 1.13 (0.06) | 0.62 (0.17) | 0.31 | 0.0 % | 0.78 |
| 1 sand | 1.13 (0.06) | 0.62 (0.17) | 0.31 | 0.0 % | 0.78 |
| 2 baseline | 0.19 (0.30) | 0.15 (0.39) | 0.58 | 13.8 % | 0.69 |
| 2 ref | 1.58 (0.15) | 0.79 (0.16) | 0.29 | 0.0 % | 0.75 |
| 2 arid | 0.68 (0.10) | 0.40 (0.27) | 0.45 | 0.0 % | 0.84 |
| 2 arid+top8 | 0.68 (0.10) | 0.40 (0.27) | 0.45 | 0.0 % | 0.84 |
| 2 sand | 0.56 (0.13) | 0.35 (0.29) | 0.47 | 0.1 % | 0.85 |

### Alps, Tyrol (N47E011)
Real: slope p50 22.35 deg, relief 1 km p50 391.1 m, flat 1.3 %.

| Site, variant | Slope (gen/real; distance) | Relief 1 km (gen/real; distance) | Relief 5 km distance | Flat | Classes L1 |
|---|---|---|---|---|---|
| 1 baseline | 0.12 (0.43) | 0.11 (0.46) | 0.57 | 0.1 % | 0.26 |
| 1 ref | 0.88 (0.09) | 0.57 (0.22) | 0.28 | 0.0 % | 0.42 |
| 1 arid | 0.87 (0.09) | 0.57 (0.22) | 0.28 | 0.0 % | 0.42 |
| 1 arid+top8 | 0.93 (0.07) | 0.63 (0.19) | 0.23 | 0.0 % | 0.37 |
| 1 sand | 0.93 (0.07) | 0.63 (0.19) | 0.23 | 0.0 % | 0.37 |
| 2 baseline | 0.11 (0.44) | 0.10 (0.46) | 0.59 | 0.0 % | 0.22 |
| 2 ref | 0.79 (0.11) | 0.48 (0.27) | 0.35 | 0.0 % | 0.50 |
| 2 arid | 0.79 (0.11) | 0.48 (0.27) | 0.35 | 0.0 % | 0.50 |
| 2 arid+top8 | 0.83 (0.10) | 0.50 (0.26) | 0.34 | 0.0 % | 0.48 |
| 2 sand | 0.83 (0.10) | 0.50 (0.26) | 0.34 | 0.0 % | 0.48 |

### Himalaya, Everest (N27E086)
Real: slope p50 29.14 deg, relief 1 km p50 523.7 m, flat 0.0 %.

| Site, variant | Slope (gen/real; distance) | Relief 1 km (gen/real; distance) | Relief 5 km distance | Flat | Classes L1 |
|---|---|---|---|---|---|
| 1 baseline | 0.23 (0.63) | 0.24 (0.78) | 0.86 | 0.0 % | 0.09 |
| 1 ref | 1.08 (0.06) | 0.87 (0.15) | 0.12 | 0.0 % | 0.28 |
| 1 arid | 1.08 (0.06) | 0.87 (0.15) | 0.12 | 0.0 % | 0.28 |
| 1 arid+top8 | 1.15 (0.13) | 0.99 (0.16) | 0.09 | 0.0 % | 0.27 |
| 1 sand | 1.15 (0.13) | 0.99 (0.16) | 0.09 | 0.0 % | 0.27 |
| 2 baseline | 0.14 (0.71) | 0.14 (0.89) | 1.01 | 0.0 % | 0.07 |
| 2 ref | 0.84 (0.18) | 0.55 (0.44) | 0.56 | 0.0 % | 0.42 |
| 2 arid | 0.84 (0.18) | 0.55 (0.44) | 0.56 | 0.0 % | 0.42 |
| 2 arid+top8 | 0.89 (0.15) | 0.59 (0.39) | 0.51 | 0.0 % | 0.39 |
| 2 sand | 0.89 (0.15) | 0.59 (0.39) | 0.51 | 0.0 % | 0.39 |

### Verdicts per variant
Criterion 1: slope and 1 km relief at 0.6 of real or more at both sites; "not worse": distances not above the baseline's.
| Landform | ref | arid | arid+top8 | sand |
|---|---|---|---|---|
| Valley and Ridge (criterion 1) | pass | pass | pass | pass |
| Black Forest (criterion 1) | pass | pass | pass | pass |
| Highlands (criterion 1) | pass | **fail** (site 2: 1 km relief 0.40; the generator puts it at 100 mm) | **fail** | **fail** |
| Alps (criterion 1) | fail (1 km relief 0.57, 0.48) | fail (0.57, 0.48) | fail (0.63, 0.50) | fail (0.63, 0.50) |
| Badlands (not worse) | worse | worse (site 2 a little less) | worse | worse |
| Erg Chebbi (not worse) | worse | worse (site 1 from 1.59-2.95 to 0.64-1.25) | worse | worse |

## Findings
1. **The arid sites get a high `k` because they lie in the generator's range zones.**
   - SPIKE-55 matched Badlands and Erg Chebbi by 1-degree mean and spread of height. Their real tiles are high plateaus (800-1,000 m).
   - The generator raises land above about 800 m only inside range zones (`5200 ridge zone inland`), so the matches land in mountain foothills: L 0.36-0.85, `k` 4.75-5.42.
   - Most of these sites are not dry in the generator (410-1,274 mm; only Erg site 1 has 179 mm).
2. **A climate (aridity) rule cannot fix them and breaks a hill site.**
   - It catches only Erg site 1 (distances 1.59-2.95 to 0.64-1.25, still worse than the baseline).
   - It flattens Highlands site 2, which the generator places in a 100 mm zone (1 km relief 0.40, criterion 1 fails).
   - The generator's climate does not follow its landforms.
3. **The generator has no high flat plateau.** Elevation and relief come from the same range term, so a high arid plateau cannot exist.
   - This is a missing landform class, not a tuning gap: it needs a plateau or arid-flat class with `k` of 1 or below that is independent of L (SPIKE-76).
4. **The Alps need `k` of about 6.5-7.8 at L 0.25-0.30** (1 km relief scales with `k`: 0.57 at 4.6, 0.48 at 4.67). The top knot at L 1 hardly reaches them (top 8 gives 0.63 and 0.50).
   - A `k` that high at L 0.25 would turn hill land into mountains (SPIKE-69's shares).
   - L does not separate the Alps (0.25-0.30) from the hills (0.07-0.18) well enough. The bake elevation does (1,330-1,370 m against 300-590 m), and so does SPIKE-69's 50 km spread (561 m at the Alps against 132-239 m).
5. **The gated floor changes nothing on the plains:** their `k` is 1.06-1.29, so the floor stays. Their flats stay at 1-2 %. The floor of 0.35 is too much with the pipeline's carve, thermal and deposition added; SPIKE-73's 0.35 was for the field alone. R7 starts lower (about 0.2).

### Inbox #17: the elevation ramps
- **(a) Interaction with `k`:**
  - The production field is `cpu-erosion-v1`; its crease amplitude `A(E) = 80 + 900 smoothstep(-200, 300 m, E)` is full from 300 m up. Every hill, mountain and arid site here is at 297 m or higher, so the ramp is full for all of them and does not separate them.
  - Only the plains near sea level get less (Brandenburg at 74-84 m: about 60 % of full).
  - The terrain field's `detail_amplitude_m` (30 to 300 m over 0-5 km) is in the reference field (`cpu-terrain-v1`), not in the production chain.
  - The arid overshoot comes from `k` (high L at the matched sites), not from a double count with the ramp.
- **(b) An elevation term inside `k`** would separate the Alps (1,330-1,370 m) from the hills (300-590 m), but it would raise the arid plateaus (800-990 m) further. It would explain the Alps' need, not fix the arid overshoot.
- **(c) An elevation-gated amplitude is not sound as a rule for relief** (argued, not measured here).
  - Relief follows tectonic and erosional setting, not height: high plateaus are flat (Tibetan plateau interior, Altiplano, Colorado Plateau, the High Plains around the Badlands), and low ground can be rugged (Appalachian Valley and Ridge at 300-600 m, coastal ranges).
  - The one sound part of the ramp is near sea level: it acts as a coast taper.
  - Relief should come from a landform or relief class, with elevation as an input to that class, not a multiplier.

## For R7 and SPIKE-76
- Not R7 tuning, a design item: the arid and plateau landforms need their own class (`k` of 1 or below, flat floors) that is independent of L and of elevation. The generator cannot make a high flat plateau today. SPIKE-76 takes this up.
- The Alps need a second driver of mountain strength besides L: the 50 km spread or a ridge term, or an authored relief class.
- The plains' floor in the pipeline: about 0.2, tuned against the carve and deposition.
- A climate rule is not a landform rule while the generator's climate is independent of its landforms.

## Open points
- `k` at Erg site 2 and Badlands site 1 is not reduced by any rule tested; only a class change can reach them.
- Seed 1; the site matching (SPIKE-55) is not changed (the planner's call: do not re-match).

## Visuals
- None: the site tables are the evidence; SPIKE-74's stills show the `ref` variant.

## Handover
- On branch `spike/m9-plains`, build the `terrain_score` example and run it with `S75_DIR=<dir>` (`proto/run.sh.txt`); about 15 min for 18 sites and 4 variants.
