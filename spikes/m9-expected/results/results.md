# SPIKE-74: M9 expected outcome

Statistics produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. Citation: https://doi.org/10.5270/ESA-c5d3d65

Opened: 2026-10-31
Question: What does the whole proposed M9 pipeline (ADR 0088 as revised by SPIKE-68 to SPIKE-73) give at every scorecard site, against today's baseline and the real tiles? What would M9 criterion 1 read, and what is left for R7?
Why it matters: a synthesis for the owner's decision on ADR 0088 on return.
Time box: 3 h (opus-planner, 2026-10-31; Opus); used about 1 h 50 min, with 25 min of machine time.
Success criteria:
- one table per landform: baseline, pipeline, real;
- the seam p99 at the four bake sites;
- stills at 300 m and 30 m per landform;
- criterion 1 stated plainly.
Code location: branch `spike/m9-plains` (pushed, no PR; commit a2c76c3c; spike code, never merged):
- the `terrain_score` example takes `S74_DIR` (`proto/s74_block.rs.txt`) with a kernels module built from SPIKE-70 to SPIKE-73 and SPIKE-64's deposition (`proto/make_module.py.txt`);
- the fine-crease floor is settable at runtime.
The seam check is an offline crate from SPIKE-73's (`proto/seam.sh.txt`).
Closed: 2026-10-31

## Method
- **Rasters:** the scorecard's (TEST-079): each landform's real GLO-30 tile (central 0.5 degree at 30 m) and the generator's two matched sites at 30 m, `global16384` read only. Baseline is today's field, the same as `docs/milestones/M9/terrain-score-baseline.md`.
- **Pipeline per site, all at 30 m on the 55 km window unless stated:**
  1. the field sampled with the fine-crease floor 0.35 (SPIKE-73);
  2. `k` from SPIKE-69's curve of the landform lattice (1 at L 0, 4.5 by L 0.028, 5.6 at L 1), times the coast taper on the 10 km mean; heights `m + k (h - m)`, `m` the 10 km box mean;
  3. pass 1: the same on a 400 km window of point samples at 814 m (SPIKE-71), carved there (transient carve with breach, fill and uplift, SPIKE-70); its change upsampled bilinearly;
  4. pass 2: the local carve, incise only, no uplift, area cap 1 km^2 (SPIKE-72, SPIKE-73);
  5. active-cell thermal at 33 degrees, 100 passes (SPIKE-71);
  6. one deposition sweep (SPIKE-64, `kc` 1e5, `zeta` 0.5).
- **Differences from the real bake:** pass 2 runs at 30 m here (100 m in the bake), on one window per site instead of per tile; no seams are involved in the scores.
- **Seams:** pass 2 as now specified (incise only, no uplift) on SPIKE-70's four bake windows at 100 m, scored as SPIKE-70 (the edge step p99 against the off-edge p99).
- **Stills:** SPIKE-65's column-cast render (not the engine's renderer) of the 30 m rasters. Rows real, baseline, pipeline (site 1); columns eye 300 m (pitch 8 degrees) and 30 m (4 degrees), heading east, sun at azimuth 290, elevation 15.
- **Runs:** BelowNormal under the heavy lock (`raw/run.log`, `raw/seam.log`).

## Results
Distances in brackets are the scorecard's (0 is equal; in units of the real p5-to-p95 range).

### Brandenburg plain (N52E012)
| Measure | Real | Site 1: baseline to pipeline | Site 2: baseline to pipeline |
|---|---|---|---|
| Slope p50 (deg; distance) | 1.15 | 0.56 (0.19) to 2.31 (0.09) | 0.56 (0.19) to 2.29 (0.10) |
| Relief 1 km p50 (m; distance) | 22.9 | 8.6 (0.35) to 24.2 (0.09) | 8.2 (0.36) to 23.2 (0.06) |
| Relief 5 km p50 (m; distance) | 51 | 30 (0.39) to 61 (0.26) | 27 (0.44) to 53 (0.13) |
| Flat share | 22.6 % | 91.0 % to 1.2 % | 89.6 % to 1.5 % |
| Classes L1 (0 to 2) | 0 | 1.39 to 0.81 | 1.38 to 0.79 |
| Drainage density ratio | 1 | 1.23 to 1.03 | 1.08 to 1.00 |

### Loess hills and plains (N41W096)
| Measure | Real | Site 1: baseline to pipeline | Site 2: baseline to pipeline |
|---|---|---|---|
| Slope p50 (deg; distance) | 3.88 | 0.85 (0.39) to 3.62 (0.18) | 0.88 (0.39) to 3.44 (0.10) |
| Relief 1 km p50 (m; distance) | 38.2 | 12.6 (0.78) to 36.9 (0.52) | 13.3 (0.76) to 35.5 (0.23) |
| Relief 5 km p50 (m; distance) | 70 | 41 (0.64) to 101 (1.01) | 43 (0.56) to 89 (0.50) |
| Flat share | 1.0 % | 52.4 % to 0.1 % | 48.9 % to 0.3 % |
| Classes L1 (0 to 2) | 0 | 1.48 to 0.62 | 1.43 to 0.63 |
| Drainage density ratio | 1 | 1.36 to 1.22 | 1.47 to 1.21 |

### Badlands (N43W102)
| Measure | Real | Site 1: baseline to pipeline | Site 2: baseline to pipeline |
|---|---|---|---|
| Slope p50 (deg; distance) | 3.04 | 2.51 (0.12) to 13.07 (0.97) | 4.21 (0.10) to 23.21 (1.70) |
| Relief 1 km p50 (m; distance) | 39.5 | 34.5 (0.11) to 116.1 (1.24) | 53.6 (0.21) to 216.3 (2.78) |
| Relief 5 km p50 (m; distance) | 108 | 104 (0.04) to 274 (1.77) | 144 (0.36) to 491 (3.93) |
| Flat share | 2.6 % | 0.0 % to 0.0 % | 0.0 % to 0.0 % |
| Classes L1 (0 to 2) | 0 | 0.31 to 0.44 | 0.23 to 0.28 |
| Drainage density ratio | 1 | 1.89 to 1.05 | 1.41 to 1.02 |

### Erg Chebbi desert (N31W004)
| Measure | Real | Site 1: baseline to pipeline | Site 2: baseline to pipeline |
|---|---|---|---|
| Slope p50 (deg; distance) | 0.86 | 2.37 (0.32) to 14.46 (2.95) | 3.15 (0.48) to 17.05 (3.36) |
| Relief 1 km p50 (m; distance) | 11.8 | 33.5 (0.26) to 135.1 (1.59) | 43.2 (0.37) to 156.7 (1.88) |
| Relief 5 km p50 (m; distance) | 41 | 106 (0.29) to 330 (1.74) | 127 (0.38) to 368 (1.98) |
| Flat share | 47.9 % | 0.0 % to 0.0 % | 0.0 % to 0.0 % |
| Classes L1 (0 to 2) | 0 | 1.28 to 1.31 | 1.30 to 1.29 |
| Drainage density ratio | 1 | 1.76 to 1.00 | 1.53 to 1.04 |

### Valley and Ridge (N40W078)
| Measure | Real | Site 1: baseline to pipeline | Site 2: baseline to pipeline |
|---|---|---|---|
| Slope p50 (deg; distance) | 8.11 | 1.43 (0.36) to 9.39 (0.04) | 1.37 (0.36) to 10.02 (0.08) |
| Relief 1 km p50 (m; distance) | 120.3 | 20.4 (0.46) to 88.3 (0.21) | 19.1 (0.46) to 96.2 (0.17) |
| Relief 5 km p50 (m; distance) | 397 | 65 (0.88) to 205 (0.46) | 62 (0.88) to 223 (0.33) |
| Flat share | 0.0 % | 4.6 % to 0.0 % | 6.1 % to 0.0 % |
| Classes L1 (0 to 2) | 0 | 0.52 to 0.63 | 0.57 to 0.62 |
| Drainage density ratio | 1 | 1.33 to 1.12 | 1.25 to 1.16 |

### Black Forest (N48E008)
| Measure | Real | Site 1: baseline to pipeline | Site 2: baseline to pipeline |
|---|---|---|---|
| Slope p50 (deg; distance) | 9.06 | 1.66 (0.35) to 12.01 (0.07) | 1.43 (0.36) to 12.42 (0.07) |
| Relief 1 km p50 (m; distance) | 140.6 | 24.2 (0.46) to 117.1 (0.17) | 22.4 (0.47) to 123.5 (0.16) |
| Relief 5 km p50 (m; distance) | 315 | 84 (0.54) to 287 (0.17) | 84 (0.54) to 305 (0.15) |
| Flat share | 0.0 % | 1.8 % to 0.0 % | 6.1 % to 0.0 % |
| Classes L1 (0 to 2) | 0 | 0.58 to 0.44 | 0.84 to 0.43 |
| Drainage density ratio | 1 | 1.45 to 1.28 | 1.35 to 1.26 |

### Scottish Highlands (N57W005)
| Measure | Real | Site 1: baseline to pipeline | Site 2: baseline to pipeline |
|---|---|---|---|
| Slope p50 (deg; distance) | 6.58 | 1.21 (0.30) to 8.99 (0.11) | 1.23 (0.30) to 10.38 (0.15) |
| Relief 1 km p50 (m; distance) | 127.1 | 18.1 (0.39) to 88.9 (0.15) | 19.3 (0.39) to 100.1 (0.16) |
| Relief 5 km p50 (m; distance) | 389 | 65 (0.58) to 221 (0.29) | 69 (0.58) to 240 (0.29) |
| Flat share | 4.9 % | 16.3 % to 0.0 % | 13.8 % to 0.0 % |
| Classes L1 (0 to 2) | 0 | 0.67 to 0.75 | 0.69 to 0.75 |
| Drainage density ratio | 1 | 0.92 to 0.89 | 0.92 to 0.87 |

### Alps, Tyrol (N47E011)
| Measure | Real | Site 1: baseline to pipeline | Site 2: baseline to pipeline |
|---|---|---|---|
| Slope p50 (deg; distance) | 22.35 | 2.65 (0.43) to 19.59 (0.09) | 2.49 (0.44) to 17.72 (0.11) |
| Relief 1 km p50 (m; distance) | 391.1 | 44.9 (0.46) to 223.9 (0.22) | 40.0 (0.46) to 187.7 (0.27) |
| Relief 5 km p50 (m; distance) | 1039 | 169 (0.57) to 603 (0.28) | 157 (0.59) to 529 (0.35) |
| Flat share | 1.3 % | 0.1 % to 0.0 % | 0.0 % to 0.0 % |
| Classes L1 (0 to 2) | 0 | 0.26 to 0.42 | 0.22 to 0.50 |
| Drainage density ratio | 1 | 1.18 to 1.08 | 1.31 to 1.09 |

### Himalaya, Everest (N27E086)
| Measure | Real | Site 1: baseline to pipeline | Site 2: baseline to pipeline |
|---|---|---|---|
| Slope p50 (deg; distance) | 29.14 | 6.78 (0.63) to 31.33 (0.06) | 4.16 (0.71) to 24.47 (0.18) |
| Relief 1 km p50 (m; distance) | 523.7 | 125.0 (0.78) to 453.1 (0.15) | 73.3 (0.89) to 289.5 (0.44) |
| Relief 5 km p50 (m; distance) | 1573 | 476 (0.86) to 1489 (0.12) | 275 (1.01) to 856 (0.56) |
| Flat share | 0.0 % | 0.0 % to 0.0 % | 0.0 % to 0.0 % |
| Classes L1 (0 to 2) | 0 | 0.09 to 0.28 | 0.07 to 0.42 |
| Drainage density ratio | 1 | 2.54 to 1.17 | 1.59 to 1.17 |

### Seams (pass 2 incise only, no uplift; edge p99 against off-edge p99)
| Site | Seam | Ratio to the joint bake (relief1 / relief5 / slope p50) |
|---|---|---|
| Highlands | 67.8 vs 67.8 m | 1.07 / 1.09 / 1.09 |
| Black Forest | 56.7 vs 58.7 m | 1.02 / 1.06 / 1.06 |
| Valley and Ridge | 48.5 vs 52.0 m | 0.96 / 0.99 / 1.01 |
| Alps | 78.2 vs 78.8 m | 1.22 / 1.08 / 1.31 |
- Seam-free at all four sites.
- Closer to the joint bake than SPIKE-72's variant with uplift (relief1 1.05-1.25, slope 1.16-1.36): the uplift artefact SPIKE-73 found is gone.

### M9 criterion 1 as it would read
Generator over real, p50 of slope and of 1 km relief (the bound is 0.6 at both sites; today 0.11-0.35):
| Landform | Site 1: slope / relief 1 km | Site 2: slope / relief 1 km | Pass |
|---|---|---|---|
| Valley and Ridge | 1.16 / 0.73 | 1.23 / 0.80 | yes |
| Black Forest | 1.32 / 0.83 | 1.37 / 0.88 | yes |
| Scottish Highlands | 1.37 / 0.70 | 1.58 / 0.79 | yes |
| Alps, Tyrol | 0.88 / **0.57** | 0.79 / **0.48** | no: 1 km relief under 0.6 |
| Badlands, desert not worse than the baseline | Badlands distances 0.04-0.36 to 0.97-3.93; Erg Chebbi 0.26-0.48 to 1.59-3.36 | | **no** |

### Look (`images/`)
- `<tile>-stills.jpg`, one per landform (rows real, baseline, pipeline; eye 300 m and 30 m):
  - `N52E012` Brandenburg, `N41W096` Loess, `N43W102` Badlands, `N31W004` Erg Chebbi;
  - `N40W078` Valley and Ridge, `N48E008` Black Forest, `N57W005` Highlands;
  - `N47E011` Alps, `N27E086` Himalaya.
- Hills and mountains: today's field is a near-flat plain to the horizon at every hill site. The pipeline shows ridges and valleys of about the real size; the Black Forest and Highlands pipelines read close to the real rows.
- Plains: the pipeline has relief like the real tile, but its ground is evenly undulating where the real tile has flat floors between features.
- Badlands and Erg Chebbi: the pipeline is clearly rougher than real (hill country where the real ground is near flat).

## Findings
1. **Hills and mountains close most of the gap.**
   - The distances for slope and 1 km relief fall from 0.30-0.89 to 0.04-0.44.
   - Valley and Ridge, Black Forest and Highlands pass criterion 1 (0.70-1.58).
   - Drainage density moves toward real (ratios 0.87-1.28, from 0.92-2.54).
2. **The Alps miss criterion 1 on 1 km relief (0.48-0.57)** while slope passes (0.79-0.88). SPIKE-69's top knot (5.6) is under the Alps' need (SPIKE-66: about 7.5). This is R7's first tuning item.
3. **Badlands and Erg Chebbi get much worse:** slope 13-23 degrees against 1-3 real; relief distances 1.2-3.9.
   - Their 1 and 5 km relief rises 3-5x, as a `k` of 4.5-5.6 would: the landform lattice is high at their matched generator sites (inferred from the rise; `k` per site was not logged).
   - The criterion's "not worse" clause fails.
   - The fix belongs to `k`: low or below-1 `k` for badland and desert landforms (R7, the item recorded in the M9 draft).
4. **The plains' 1 km relief now matches** (Brandenburg 23-24 against 23 m, Loess 36-37 against 38 m), and class L1 falls from 1.38-1.48 to 0.62-0.81.
   - But the flats overshoot: 0.1-1.5 % against 1-23 % real, and slope is 2x at Brandenburg.
   - The floor 0.35 and `k` add up, because the sites' L is above 0: R7 tunes the floor against `k`.
5. **The class mix (L1) gets worse at the mountain sites** (Alps 0.22-0.26 to 0.42-0.50; Himalaya 0.07-0.09 to 0.28-0.42): steeper, but fewer of the real ridge and valley classes.
6. **Seams are clean at all four bake sites** with pass 2 as specified now (incise only, no uplift).

## What R7 has to tune
| Item | From | Target |
|---|---|---|
| `k` top knot or a ridge term for high mountains | Alps 1 km relief 0.48-0.57 | 0.6 or more (0.8 the aim) |
| `k` for badlands and deserts: low or under 1 | Badlands and Erg Chebbi distances 1-4 | not worse than the baseline (0.04-0.48) |
| The fine-crease floor against `k` on plains | Brandenburg flat 1 %, slope 2x | flat about 23 %, slope about 1.15 degrees |
| The class mix in mountains | L1 0.28-0.50 | toward the baseline's 0.07-0.26, without losing relief |
| The carve's `K t` and deposition `kc` | flats and density | real flats and density 1 +- 0.3 |

## Open points
- Pass 2 ran at 30 m on one window per site, not at 100 m per tile; the bake's results will differ in detail.
- The stills are a column-cast render of the rasters, not the engine. With the switch on, the engine's stills come with R4 PR 7.
- One run, seed 1, the scorecard's 18 sites.

## Visuals
- `images/<tile>-stills.jpg` for the nine landforms (about 80-105 KB each).

## Handover
- **Pipeline run:** on branch `spike/m9-plains`, build the `terrain_score` example and run it with `S74_DIR=<dir>` (`proto/run.sh.txt`). It writes `<tile>-site<k>-baseline|pipeline.f32` and `<tile>-real.f32`.
- **Stills:** `python -I proto/stills.py.txt <dir> <sheet dir> <tile> "<title>"`.
- **Seams:** `proto/seam.sh.txt` (SPIKE-73's crate with the incise-only, no-uplift pass 2) on SPIKE-70's windows.
