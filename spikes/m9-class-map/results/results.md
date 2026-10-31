# SPIKE-77: the default relief-class map and the plateau rule

Statistics produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. Citation: https://doi.org/10.5270/ESA-c5d3d65

Opened: 2026-10-31
Question: SPIKE-76's two open inputs.
- **The class map:** can a whole-planet default class map from L, a mountain-strength driver and a plateau rule match Earth's class shares, with smooth `k` across class borders?
- **The plateau rule:** a base elevation not tied to coast distance. Where do plateaus appear, and how much land lies above 1 km against Earth?
- **The arid sites:** do `k` under 1 or a gentler carve make Badlands and Erg Chebbi "not worse"?
- **The preview:** how fast is a whole-planet pre-bake preview?
Why it matters: the relief-class proposal (SPIKE-76) needs a default map before R4 PR 1; the owner's global-authoring idea needs the preview.
Time box: 3 h (opus-planner, 2026-10-31); used about 1 h 15 min, with 10 min of machine time.
Success criteria:
- class shares against Earth's;
- class and `k` maps with smooth borders;
- plateaus and land above 1 km against Earth;
- the arid sites scored;
- the preview time.
Code location: the class map is a Python prototype (`proto/classmap.py.txt`) on SPIKE-69's 0.5-degree map of the generator's inputs. The arid sites and the preview timing are on branch `spike/m9-plains` (commit fb416de2; `S77_DIR` in the `terrain_score` example, `proto/s77_block.rs.txt`; a carve-strength switch in the kernels). Spike code, never merged.
Closed: 2026-10-31

## Method
- **Class map** (on SPIKE-69's 720 x 360 map; shares on land under 60 degrees latitude, area-weighted):
  - `k` from L through SPIKE-69's knots (1, 4.5, 4.5, 5.6), with the coast taper;
  - **mountain strength** `M = smoothstep(300, 600 m, 50 km spread of the bake)`. It adds up to +3 to `k` where L is above 0 (Alps 561 m against hill sites 132-239 m, SPIKE-75);
  - **plateau rule** (a prototype layer):
    - `P = smoothstep(0.50, 0.62, craton noise) x smoothstep(0.2, 0.6, inland) x (1 - smoothstep(0.2, 0.6, zone))`;
    - the craton noise is a new fBm of the direction at frequency 1.6 with 3 octaves;
    - the base elevation gains `1200 P` m;
    - on a plateau (`Q = P (1 - M)`), `k` is held at 1.
  - **Classes** by 7 km-radius relief `k R14` as in SPIKE-69: plains under 100 m, hills to 300 m, mountains above.
- **Plateau settings tried:**
  | Plateau window of the noise | Uplift |
  |---|---|
  | 0.56-0.70 | 1,200 m |
  | 0.50-0.62 (chosen) | 1,200 m |
  | 0.46-0.58 | 1,200 m |
  | 0.46-0.58 | 1,500 m |
- **Earth's hypsometry:** land above 0.5 / 1 / 2 km = 46.7 / 27.2 / 11.7 % (Steve Dutch's table of ETOPO5 at 5 minutes, including the Antarctic and Greenland ice). Without Antarctica the share above 1 km is about 20 %; that is an estimate, not read from a table.
- **Arid sites:** SPIKE-76's class pipeline at the Badlands and Erg Chebbi sites with `k` 1, 0.6 or 0.35, and the carve's `K dt` at 100 % or 50 %.
- **Preview:** the landform lattice built for a new seed, and `global::sample` plus the lattice read on cube grids of 625^2 and 2,500^2 per face, 8 threads.
- **Runs:** BelowNormal under the heavy lock (`raw/run.log`).

## Results
### The class map (`images/classes-and-k.png`)
| Variant | Plains / hills / mountains (high) | dk p99 / max per 0.5 degree | Land above 0.5 / 1 / 2 km |
|---|---|---|---|
| SPIKE-69 (`k` from L) | 0.626 / 0.124 / 0.250 (0.061) | 1.60 / 3.50 | 0.423 / 0.121 / 0.043 |
| + mountain strength | 0.626 / 0.124 / 0.250 (0.061) | 1.74 / 5.05 | 0.423 / 0.121 / 0.043 |
| **+ plateau rule (0.50-0.62, 1,200 m)** | **0.632 / 0.135 / 0.233 (0.060)** | 1.70 / 5.05 | **0.512 / 0.256 / 0.051** |
| Earth (SPIKE-69's targets; hypsometry with ice) | 0.61 / 0.14 / 0.25 | - | 0.467 / 0.272 / 0.117 |
- **Mountain strength** raises `k` where the 50 km spread is high (the Alps' need, SPIKE-75) without moving the class shares: those cells are mountains already. The largest step of `k` rises to 5.05 per 55 km, at the edges of the high-mountain spots.
- **Plateaus** cover 12.7 % of land, inland and away from the belts. Their base elevation goes from 445 m to 1,516 m (p50) with unchanged 7 km relief (p50 67 m): they are high and flat.
- In the image, the left half shows the classes (plain, rolling, hills brown, high mountains white, plateau tan) and the right half shows `k` from 1 to 8.5. `k` varies smoothly across class borders: the memberships are smoothsteps.

Plateau settings:
| Noise window, uplift | Plateau share of land | Land above 1 km | Land above 2 km |
|---|---|---|---|
| 0.56-0.70, 1,200 m | 2.6 % | 15.8 % | 4.4 % |
| **0.50-0.62, 1,200 m** | **12.7 %** | **25.6 %** | 5.1 % |
| 0.46-0.58, 1,200 m | 18.2 % | 31.0 % | 7.4 % |
| 0.46-0.58, 1,500 m | 18.2 % | 32.0 % | 11.6 % |
- `images/plateau-elevation.png`: the base elevation (0 to 3 km) today (left) and with the chosen plateau rule (right).

### The arid sites (generator over real; distances in brackets; the baseline is today's field)
| Site | Variant | Slope | Relief 1 km | Relief 5 km distance | Flat (real) | Classes L1 |
|---|---|---|---|---|---|---|
| Erg Chebbi 1 | baseline | 2.75 (0.32) | 2.83 (0.26) | 0.29 | 0.0 % (47.9 %) | 1.28 |
| Erg Chebbi 1 | **k 0.35, carve 50 %** | 1.70 (0.16) | 1.76 (0.14) | 0.18 | 8.7 % | 0.89 |
| Erg Chebbi 2 | baseline | 3.67 (0.48) | 3.65 (0.37) | 0.38 | 0.0 % | 1.30 |
| Erg Chebbi 2 | **k 0.35, carve 50 %** | 1.50 (0.13) | 1.16 (0.10) | 0.17 | 13.6 % | 0.70 |
| Badlands 1 | baseline | 0.83 (0.12) | 0.87 (0.11) | 0.04 | 0.0 % (2.6 %) | 0.31 |
| Badlands 1 | k 1, carve 50 % | 0.83 (0.10) | 0.62 (0.23) | 0.51 | 0.2 % | 0.33 |
| Badlands 1 | k 0.6, carve 50 % | 0.51 (0.21) | 0.39 (0.38) | 0.73 | 4.3 % | 0.37 |
| Badlands 2 | baseline | 1.39 (0.10) | 1.36 (0.21) | 0.36 | 0.0 % | 0.23 |
| Badlands 2 | **k 0.6, carve 50 %** | 1.07 (0.04) | 0.95 (0.08) | 0.10 | 0.4 % | 0.47 |
| Badlands 2 | k 1, carve 50 % | 1.69 (0.19) | 1.48 (0.31) | 0.45 | 0.1 % | 0.48 |
- The other combinations are in `raw/run.log`.

### The preview
| Step | Time (8 threads) |
|---|---|
| Landform lattice, 257^2 x 6 points (a new seed) | 0.08 s |
| Global-coarse sample and landform, 2.3 M points (625^2 per face, about 16 km) | 0.7 s |
| The same, 37.5 M points (2,500^2 per face, about 4 km) | 11.5 s |
| The class map from these (Python on 0.26 M points, numpy) | 0.7 s |
- A whole-planet preview of classes and `k` takes about 1-2 s at 16 km and about 12 s at 4 km. It needs no bake: everything is a pure function of the seed (and of the authored overrides).

## Findings
1. **A default class map from L, mountain strength and a plateau rule matches Earth's class shares** (plains, hills, mountains 63 / 14 / 23 % against 61 / 14 / 25 %).
   - `k` stays continuous (p99 step 1.7 per 55 km).
   - Mountain strength lifts the Alps-type spots without moving the shares.
2. **The plateau rule makes high flat ground:**
   - 12.7 % of land becomes plateau at about 1.5 km with unchanged low relief;
   - land above 1 km rises from 12.1 % to 25.6 % (Earth 27.2 % with ice, about 20 % without Antarctica);
   - land above 2 km stays low (5.1 % against 11.7 %), because the belts give little high ground;
   - it needs a change of the global bake's base elevation (a version bump and a re-bake).
3. **Erg Chebbi is "not worse" with a sand-sea class:** `k` 0.35 and a carve at half strength make both sites better than the baseline on every distance (class L1 from 1.28-1.30 to 0.70-0.89).
4. **The Badlands cannot be made "not worse" at both sites with one class:**
   - site 1 is already close to real today (distances 0.04-0.12), and every pipeline setting adds 5 km relief there;
   - site 2 is better than the baseline with `k` 0.6 and the half carve.
   - The two matched sites differ: one fits a gentle class, the other a rugged one.
   - The class map could give each its own class. An authored or plateau class for site 1 that skips the carve is the likely answer (not tested: a carve strength of 0).
5. **The pre-bake preview is cheap:** 1-2 s for the whole planet at 16 km, about 12 s at 4 km. The owner's global-authoring step costs seconds.

## Open points
- **The arid sites were not re-matched to plateau ground:** the plateau rule is a Python prototype and the store has no plateaus. That needs the global bake changed.
- **The arid class has no default rule:** the generator's precipitation is independent of its landforms (SPIKE-75). Sand sea and badlands are authored classes, or need a climate tied to the plateau rule (dry interiors of plateaus).
- **Plateau edges:** the uplift's escarpments add relief at the plateau borders; the 7 km relief of the map was not recomputed with the uplift.
- **Land above 2 km** stays under Earth's (5 % against 12 %): the belts' own height is a separate item.
- Seed 1; the prototype's noise is not the generator's.

## Visuals
- `images/classes-and-k.png`: the default class map (left) and `k` (right), equirectangular, 0.5 degree.
- `images/plateau-elevation.png`: the base elevation today (left) and with the plateau rule (right).

## Handover
- **Class map:** `python -I proto/classmap.py.txt <SPIKE-69 map13.f32> <out>`; the plateau window and uplift come from `S77_PLO`, `S77_PHI` and `S77_UP`.
- **Arid sites and preview:** on branch `spike/m9-plains`, build the `terrain_score` example and run it with `S77_DIR=<dir>` (`proto/run.sh.txt`).
