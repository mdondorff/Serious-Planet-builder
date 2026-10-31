# SPIKE-65 Where M9's relief comes from

Opened: 2026-10-31
Question: Which lever brings the hill and mountain relief to within criterion 1's 0.6 of real:
- (a) the base field's amplitude;
- (b) stream power with uplift towards steady state;
- (c) both together.

Which of them should the R4 bake own and which the landform drivers, and at what cost per tile?
Why it matters: criterion 1 is M9's largest risk (today 0.11-0.35 of real, SPIKE-55); SPIKE-64 showed that thermal and deposition do not raise relief.
Time box: 3 h at BelowNormal, no store writes (opus-planner, 2026-10-31); used about 1.5 h, with 25 min of machine time.
Success criteria: slope p50/p99 and 1 km relief p50 against GLO-30 at about 100 m per variant; stills at 300 m and 30 m; a named lever; cost per tile.
Fallback if it fails: name the variant closest to real and what it misses.
Code location:
- Branch `spike/m9-relief-source` (pushed, no PR; commit 90196655): the ignored test `relief_window_dump` in `globalperf.rs` samples the windows.
- `proto/s65.rs.txt` holds the variants: a standalone program with no dependencies.
- `proto/score.py.txt`, `proto/render.py.txt` and `proto/stills.py.txt` hold the scoring, the stills renderer and the sheets.
Closed: 2026-10-31

## Method
**Sites:** SPIKE-55's first generator match for four real landforms.
- Each is a window of 2,560^2 cells (about 260 km) of the base field (`global16384`, no chunk stage, read only), on SPIKE-61's grid of 98-102 m.
- The windows are written to `C:/DEV/SeriousPlanetData/spike-r3/relief/` (not the store).
| Site | Latitude, longitude | Land | Heights |
|---|---|---|---|
| Alps | 11.5, -18.5 | 99 % | -26 to 4,217 m |
| Black Forest | 2.5, -124.5 | 100 % | 5 to 1,978 m |
| Highlands | 10.5, -122.5 | 89 % | -41 to 2,058 m |
| Valley and Ridge | 16.5, -111.5 | 96 % | -38 to 802 m |

**Variants:**
| Id | What |
|---|---|
| ampK | (a): land heights `m + k (h - m)`, `m` the 20 km box mean; k 2, 4, 6. Relief under about 20 km is scaled; the mean elevation and the coast stay. |
| spuR | (b): 60 implicit stream-power steps with uplift on land (Braun and Willett, m 0.5, n 1), K dt 1, U/K = R in 5, 10, 20, 40. Re-routed every 5th step (Barnes flood, jittered D8, order from the receivers; SPIKE-62); the window edge and the sea are base level. 60 steps reach steady state: the result no longer depends on the start. |
| amp2-spu10 | (c) as briefed: amp2, then spu10 (steady state). |
| ampK-spt | (c) transient: ampK, then 20 stream-power steps at U/K 20 with K dt 0.05 (K t = 1, far from steady state). |
| ...-th | then 100 thermal passes at 33 degrees (SPIKE-64's kernel). |

**Scoring:**
- The central 2,048^2 of each window (the 256-cell border left out), land only, as SPIKE-64.
- Slope from central differences; 1 km and 5 km relief as the median of block max minus min.
- The real targets are SPIKE-55's GLO-30 tiles, block-averaged to 90-113 m (SPIKE-64's `real.json`).

**Stills:** a column-cast perspective render of the raster (not the engine).
- The eye is at the highest original cell within 10 km of the centre, at 300 m (pitch 8 degrees down) and at 30 m (pitch 4), looking east.
- Sun at azimuth 290, elevation 15 degrees; grass turns to rock above 25 degrees; haze.

**Machine:** heavy lock, BelowNormal.
| Run | Time | CPU before |
|---|---|---|
| Windows | 08:58 | 1 % |
| Run 1 | 08:59-09:06 | 38 % |
| Run 2 | 09:07-09:21 | 8 % |

## Results
### Against real at about 100 m (generator / real; criterion 1 asks at least 0.6)
| Site, variant | Slope p50 | Slope p99 | Relief 1 km p50 | Relief 5 km p50 | Flat (real) |
|---|---|---|---|---|---|
| **Black Forest** (real 5.4 deg, 28.4 deg, 95 m, 256 m) | | | | | (9.4 %) |
| orig | 0.21 | 0.17 | 0.22 | 0.28 | 41.9 % |
| amp4 | 0.84 | 0.64 | 0.84 | 1.01 | 4.5 % |
| amp6 | 1.25 | 0.93 | 1.26 | 1.49 | 3.0 % |
| spu20 | 1.37 | 0.52 | 0.84 | 0.57 | 1.5 % |
| amp6-spt-th | **1.04** | **1.05** | **1.04** | **1.13** | 21.1 % |
| **Highlands** (real 6.6, 31.1, 129, 431) | | | | | (6.4 %) |
| orig | 0.13 | 0.13 | 0.12 | 0.15 | 57.5 % |
| amp6 | 0.71 | 0.72 | 0.68 | 0.69 | 10.4 % |
| spu40 | 2.21 | 0.84 | 1.23 | 0.67 | 0.5 % |
| amp6-spt-th | **0.74** | **0.98** | **0.67** | **0.62** | 18.1 % |
| **Valley and Ridge** (real 5.7, 25.6, 99, 335) | | | | | (6.3 %) |
| orig | 0.17 | 0.14 | 0.18 | 0.18 | 52.7 % |
| amp6 | 0.97 | 0.78 | 1.01 | 0.94 | 7.6 % |
| spu20 | 1.31 | 0.54 | 0.80 | 0.43 | 1.4 % |
| amp6-spt-th | **0.84** | **1.01** | **0.85** | **0.74** | 20.8 % |
| **Alps** (real 16.9, 49.0, 345, 1,029) | | | | | (8.9 %) |
| orig | 0.12 | 0.31 | 0.11 | 0.15 | 20.7 % |
| amp6 | 0.69 | 1.18 | 0.63 | 0.68 | 1.7 % |
| spu40 | 0.86 | 0.55 | 0.46 | 0.28 | 0.5 % |
| amp6-th | 0.69 | 0.98 | 0.63 | 0.68 | 1.7 % |
| amp6-spt-th | 0.60 | 0.78 | 0.53 | 0.58 | 16.2 % |

All rows, including amp2/amp3/amp4, the other U/K values and the -th and -spt variants, are in `raw/run1.json` and `raw/run2.json`.

### What each lever does
- **(a) Amplitude:**
  - Slope and relief scale linearly with k at every scale, and each site keeps its own character.
  - The base's slope spread (p99/p50 of about 5-7) is wider than real hills (4-5), so p99 lags p50 for the hills and overshoots for the Alps (amp6 p99 58 degrees against 49).
  - The mean elevation stays; the maximum rises moderately (the Alps window from 3.8 to 5.6 km at k 6).
- **(b) Stream power to steady state:**
  - It forgets the start: the four sites give the same numbers at one U/K. At spu40, slope p50 is 14.6-14.7 degrees and 1 km relief 158-159 m everywhere.
  - Slope rises with U/K as expected (p50 about 0.37 U/K degrees), but its spread is narrow (p99/p50 about 1.8).
  - 5 km relief stays low against 1 km relief (ratio 1.85, real 2.6-3.4): uniform dissection without large-scale relief.
  - It also erodes the mean elevation down by 300 to 1,100 m.
  - (c) as briefed (amp2, then spu10 to steady state) equals spu10: the amplitude is forgotten.
- **(c) transient (amplitude, then a short carve, then thermal):**
  - The carve cuts valleys into the amplified ground and raises the slope tail: p99 0.98-1.05 of real at the three hill sites.
  - Thermal caps the tail: Alps p99 from 64 to 38 degrees, share over 33 degrees from 12.7 % to 6.1 %.
  - Side effect: the flat share rises to 16-21 % (real 6-9 %). These are flooded valley floors and pits of the carve.
- **Stills** (`images/`):
  - The original ground reads as a plain at 300 m.
  - amp6 reads as hill country, with mountains at the Alps site.
  - spu40 is an even, egg-crate dissection.
  - amp6-spt-th has ridges and incised valleys.
  - At 30 m the 100 m cells show as facets in every variant; finer relief comes from the field's octaves.

### Cost per window (2,560^2, 6.6 M cells; a SPIKE-61 tile of 12.8 M cells is about twice this)
| Step | Time |
|---|---|
| Amplitude (box mean and scale) | 0.04 s |
| Transient carve, 20 steps with 4 routes | 4.1-4.7 s routing, 0.9-1.1 s steps |
| Steady-state stream power, 60 steps with 12 routes | 16-19 s routing, 2.6-3.1 s steps |
| Thermal, 100 passes on 4 threads | 7.4-11.4 s |
- For the planet's 2,140 land tiles at 4 tiles at a time:
  - transient carve about 10 s per tile, about 1.5 h;
  - thermal about 16-22 s per tile, about 2.5-3 h at full passes, less with SPIKE-64's active-cell kernel;
  - steady state about 40 s per tile, about 6 h.

## Findings
1. **The relief comes from the amplitude (a).** It is the only lever that keeps each site's landform and scales slope and relief together. Amplitude x4-6 brings the three hill sites to 0.68-1.26 of real; the Alps need more than x6 (slope p50 and relief 0.63-0.69 at x6). Cost is nil.
2. **Stream power to steady state (b) is the wrong lever for relief.** It erases the landform (four sites, one answer), its scale ratio is wrong (5 km against 1 km relief too low), and it erodes the mean elevation.
3. **A transient carve after the amplitude shapes the slope tail;** thermal caps it.
   - amp6-spt-th meets criterion 1 at the Black Forest (1.04-1.13), Valley and Ridge (0.74-1.01) and Highlands (0.62-0.98).
   - The Alps reach 0.53-0.78.
   - It needs a fix for the flats it leaves (16-21 % against 6-9 %).
4. **Ownership:**
   - **The landform drivers own the amplitude:** a relief factor per landform (about x4-6 for hills, more for mountains) that scales the base field's sub-20 km relief, in the field and therefore in every consumer.
   - **The R4 bake owns the transient carve** (few steps, K t about 1, not steady state) **and thermal**; both are about 15-30 s per SPIKE-61 tile.
   - Steady-state stream power is not part of the plan.
5. **What to check next:**
   - The amplitude in the generator itself (the bake relief and crease amplitude per landform, not a post-scale), at the scorecard's sites.
   - The flats the carve leaves.
   - The Alps' factor.

## Open points
- The amplitude is a post-scale of the sampled field about a 20 km mean. The generator's own constants (bake relief, crease amplitude, landform lift) are where R4 and the landform drivers would put it; their effect on bounds, held-back tiles and the coast is not tested.
- One site per landform; SPIKE-55's second matches are not run.
- The stills are a column-cast render of the rasters, not the engine; the engine's stills need a store with the variant (R4).
- The stream-power parameters are one choice (m 0.5, n 1, uniform uplift); spatially varying uplift (GEN-063's stationary uplift) was not tried.
- The window spacing is 98-102 m by site; the code uses 101.8 m throughout (an error of up to 4 % in slope).
- The 20 km mean is a box mean; relief at wavelengths over 20 km is not scaled (5 km relief is within it).

## Visuals
- `images/blackforest-stills.jpg`: Black Forest site; rows orig, amp6, spu20, amp6-spt-th; columns eye 300 m and 30 m.
- `images/alps-stills.jpg`: Alps site; rows orig, amp6, spu40, amp6-spt-th.

Generator data only (no GLO-30); not the engine's renderer.

## Handover
1. **Windows:** run from the worktree root on branch `spike/m9-relief-source`:
   ```
   PLANET_ANET_STORE=C:/DEV/SeriousPlanetData/global16384 PLANET_ANET_RES=16384 RELIEF_SITES=alps:11.5:-18.5,blackforest:2.5:-124.5,highlands:10.5:-122.5,valleyridge:16.5:-111.5 RELIEF_W=2560 RELIEF_DIR=C:/DEV/SeriousPlanetData/spike-r3/relief cargo test -p planet --release --lib relief_window_dump -- --ignored --nocapture
   ```
2. **Variants:** build `proto/s65.rs.txt` as a standalone crate, then run `s65 <relief dir> <out> alps,blackforest,highlands,valleyridge`; `S65_MODE=2` runs the transient and thermal variants.
3. **Score:** `python -I score.py <out> <sites> <variants>`.
4. **Stills:** `python -I stills.py <out> <sheet dir> <site> <variants>`.
- The windows (100 MB) stay in `spike-r3/relief/`, beside SPIKE-61's dumps.
