# SPIKE-81: relief classes from physical drivers

Statistics produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved. The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30. Citation: https://doi.org/10.5270/ESA-c5d3d65

Opened: 2026-10-31
Question: can the default relief class be derived from physical drivers through a rule table (ADR 0076: drivers → rules → consumers), instead of SPIKE-77/80's rules of thumb? The drivers: plate history, an analytic rock type, climate (aridity) and height. The Badlands, which SPIKE-80's rules miss, are the key test. A paragraph says what a soil-depth driver would need.
Why it matters: the owner's principle (ADR 0076) is physical drivers and rules, not painted types; the class map of decision 2 is the place where it applies to relief.
Time box: 2.5 h (opus-planner, 2026-10-31); used about 1 h 10 min, with 10 min of machine time.
Success criteria:
- a rule table from drivers to class weights;
- class shares against Earth and SPIKE-80;
- the scorecard at re-matched sites, Badlands first;
- the soil-depth paragraph.
Code location:
- `proto/classdrivers.py.txt` runs SPIKE-80's `combined.py` and adds the drivers, the rule table and the re-match;
- branch `spike/m9-plains` (commit 9af5d4e2) adds `S81_CLIMATE` (a dump of the generator's precipitation and temperature at 0.25 degree) and a 4-channel field for the S80 site block (`S80_NCH=4`: the class's carve strength); `proto/run.sh.txt` runs it.
Spike code, never merged.
Closed: 2026-10-31

## Method
- **Drivers** per 0.25-degree cell, on SPIKE-80's combined model:
  | Driver | Definition | Source |
  |---|---|---|
  | Orogen core, flank, old margin, back-arc | SPIKE-80's plate memberships | plate history |
  | Craton | `smoothstep(500, 1,100 km)` of the distance to the nearest plate margin | plate history |
  | Rift | divergent margins (convergence under -0.15), 150 km half-width | plate history |
  | Basin | low (`1 - smoothstep(200, 700 m)` of the elevation) and inland | height |
  | Rock hardness `H` | `0.45 + 0.4 craton (1 - basin) + 0.35 core - 0.4 foreland - 0.35 basin + 0.25 noise`, clamped to `[0, 1]`; soft = `1 - H` | analytic lithology (ADR 0076), built from the above |
  | Aridity | aridity index `AI = P / PET` with `PET = 300 + 45 max(T, 0)` mm (a stand-in), `arid = 1 - smoothstep(0.2, 0.65, AI)` (UNEP's arid and humid bounds) | the generator's own precipitation `P` and temperature `T` |
  | Height | the combined elevation; SPIKE-77's plateau field | SPIKE-80 |
- **The rule table** (raw weights, each a product of response curves; trapezoid `trap(a, b, c, d)`):
  | Class | Rule | `k` | Floor | Carve |
  |---|---|---|---|---|
  | High mountains | core × `smoothstep(2, 3 km)` of height | 7.5 | 0 | full |
  | Mountains | core × (1 - that) | 6 | 0 | full |
  | **Badlands** | not core × soft (`smoothstep(0.45, 0.65)`) × semi-arid (`trap(0.2, 0.4, 0.8, 0.95)` of `arid`) × moderate height (`trap(250, 450, 1,500, 2,200 m)`) | 2 | 0 | full |
  | **Sand sea** | not core × arid (`smoothstep(0.75, 0.92)`) × soft or basin (`smoothstep(0.4, 0.6)`) × under 1.3-1.9 km × not a flank | 0.35 | 0 | half |
  | Hills | the rest × flank or old margin (`smoothstep(0.2, 0.45)`) | 5 | 0 | full |
  | Plateau | the rest × plateau field or back-arc (`smoothstep(0.3, 0.6)`) | 1 | 0 | full |
  | Rolling plain | the rest × `smoothstep(0.2, 0.5)` of `0.6 soft + 0.4 rift + 0.3 humid - 0.3` | 1 | 0.35 | full |
  | Plain | the rest | 1 | 0.2 | full |
  - Memberships sum to 1 per cell. `k`, the crease floor and the carve strength are membership-weighted; the class hill band follows `k` (SPIKE-80: 30 m, 175 m at `k` 5, 825 m at `k` 7.5).
  - The `k` of the new classes is set a priori, not fitted: badlands 2 (rugged at small scale, low relief), sand sea 0.35 with half the carve (SPIKE-77).
- **Sites re-matched** (`raw/sites-derived.toml`) by derived class and the real window's mean height, with SPIKE-80's limits (well inland; hills and mountains on today's ground at 300 m or more). Badlands go to the badlands class, Erg Chebbi to the sand sea.
- **Site pipeline:** SPIKE-80's (the delta on today's stored field, `k` and floor per cell, the two passes, thermal, deposition), with the class hill band and the carve strength of the class at the site centre (`derived+hb`).
- **Runs:** BelowNormal under the heavy lock (`raw/run.log`, `raw/summary.txt`, `raw/classdrivers.log`).

## Results
### Class shares (land under 60 degrees)
| | Plain | Rolling plain | Plateau | Sand sea | Badlands | Hills | Mountains (high) |
|---|---|---|---|---|---|---|---|
| Derived, by largest membership | 32.2 % | 22.4 % | 13.3 % | 7.9 % | 3.3 % | 15.3 % | 5.5 % (4.0) |
| SPIKE-80 rules | 32.3 % | 29.4 % | 13.8 % | - | - | 18.9 % | 5.5 % (4.0) |

| Relief classes (`k R14`) | Plains | Hills | Mountains |
|---|---|---|---|
| Derived | 62.1 % | 21.7 % | 16.2 % |
| SPIKE-80 rules | 60.8 % | 20.0 % | 19.1 % |
| Earth (SPIKE-69's targets) | 61 % | 14 % | 25 % |
- The derived classes keep the relief shares within 3 points of SPIKE-80's. Mountains lose 3 points: badlands at `k` 2 replace some hill ground.
- Dry land: 26 % of land has `arid` over 0.5 (AI under about 0.4) and 18 % over 0.9 (AI under about 0.25). The generator's aridity is latitude-banded (dry belts near 25 degrees), so badlands and sand seas sit there (`images/derived-classes-and-drivers.png`).
- Rock: 32 % of land is hard (`H` over 0.6) and 39 % soft (under 0.4).
- `k` steps up to 7.1 per 28 km (p99 1.9), as in SPIKE-80: the belt edges.
- `images/derived-classes-and-drivers.png`: left, the derived classes (as SPIKE-80, plus badlands red and sand sea yellow); right, the drivers (red = arid on soft rock, yellow-orange = arid, blue = hard and humid, green = soft and humid).

### Scorecard (slope / 1 km relief, generator over real, site 1, site 2; distances in brackets)
| Landform | Baseline (today, SPIKE-55's sites) | SPIKE-80 rules (`+hb` or authored) | **SPIKE-81 derived** | Verdict for derived |
|---|---|---|---|---|
| **Badlands** | 0.83 / 0.87 (0.12 / 0.11), 1.39 / 1.36 (0.10 / 0.21) | authored: 0.24 / 0.34 (0.30 / 0.44), 0.26 / 0.31 (0.30 / 0.45) | **1.34 / 1.20 (0.14 / 0.12), 1.30 / 1.16 (0.15 / 0.14)** | about equal to the baseline (site 1 within 0.02; site 2 worse on slope, better on relief); much better than SPIKE-80; classes L1 0.60-0.63 against 0.23-0.31 |
| Erg Chebbi | 2.75 / 2.83 (0.32 / 0.26), 3.67 / 3.65 (0.48 / 0.37) | authored: 0.95 / 1.12 (0.06 / 0.07), 0.98 / 0.98 (0.06 / 0.07) | 1.89 / 1.88 (0.19 / 0.16), 2.29 / 2.70 (0.48 / 0.37) | not worse than the baseline; far behind the authored class |
| Brandenburg plain | 0.49 / 0.38, 0.49 / 0.36 | 1.64 / 1.02, 1.66 / 1.01 | 1.86 / 1.10 (0.09 / 0.10), 1.86 / 1.13 (0.08 / 0.11) | better than the baseline; flat 2.6 % against 22.6 % |
| Loess | 0.22 / 0.33, 0.23 / 0.35 | 0.76 / 0.79, 0.65 / 0.64 | 1.61 / 1.89 (0.48 / 1.26), 0.72 / 0.74 (0.10 / 0.27) | site 1 overshoots (on today's rough ground) |
| Valley and Ridge | 0.18 / 0.17, 0.17 / 0.16 | 2.08 / 1.72, 2.78 / 2.89 | 1.93 / 1.55, 2.38 / 2.28 | criterion 1 pass; overshoots |
| Black Forest | 0.18 / 0.17, 0.16 / 0.16 | 1.12 / 0.87, 0.98 / 0.73 | 0.95 / 0.72 (0.07 / 0.17), 1.00 / 0.75 (0.06 / 0.15) | criterion 1 pass |
| Highlands | 0.18 / 0.14, 0.19 / 0.15 | 3.10 / 1.97, 1.87 / 1.08 | 2.63 / 1.65, 1.38 / 0.80 (0.13 / 0.10) | criterion 1 pass; site 1 overshoots |
| Alps | 0.12 / 0.11, 0.11 / 0.10 | 1.03 / 0.72, 0.68 / 0.47 | **1.03 / 0.71 (0.04 / 0.16), 1.02 / 0.72 (0.05 / 0.16)** | **criterion 1 pass** at both sites |
| Himalaya | 0.23 / 0.24, 0.14 / 0.14 | 1.15 / 0.98, 1.12 / 0.92 | 0.97 / 0.77 (0.10 / 0.21), 1.15 / 0.98 (0.17 / 0.12) | better than the baseline |
- **The Badlands key test:**
  - the drivers put the Badlands class where the rock is soft, the climate semi-arid (405-482 mm at 18-21 °C) and the ground about 800 m high;
  - there the pipeline gives 1.2-1.3 times real slope and relief, about the baseline's distances, and far better than SPIKE-80's authored no-carve class (0.30-0.45) on flat lowland;
  - the landform classes are worse than the baseline (L1 0.60-0.63 against 0.23-0.31).
- **The sand sea** overshoots: at the matched sites `k` is 1.13 and 0.46, not 0.35, because the class is blended with plain within the cell. The rule needs a sharper arid response or a wider sand-sea core. The authored class (SPIKE-80) stays the better result.
- **The Alps pass criterion 1 at both sites** with the derived classes (0.71 and 0.72). Their re-matched sites moved to cores where today's ground is at 300 m or more, and the class hill band does the rest.
- The overshoots at Valley and Ridge, the Highlands site 1 and Loess site 1 are SPIKE-80's stand-in effect: those sites stand on today's rough or high ground.

### Stills
- `images/N43W102-stills.jpg` (Badlands) and `images/N31W004-stills.jpg` (Erg Chebbi): rows real, baseline (today at SPIKE-55's site 1), derived classes (re-matched site 1); eye 300 m and 30 m.

## Findings
1. **The default class can be derived from physical drivers.** A rule table of eight classes over plate history, an analytic rock hardness, aridity from the generator's climate and height keeps the relief shares (62 / 22 / 16 % against SPIKE-80's 61 / 20 / 19 % and Earth's 61 / 14 / 25 %) and adds two classes that SPIKE-80's rules lacked: badlands (3.3 % of land) and sand sea (7.9 %).
2. **The Badlands pass the key test:**
   - derived badlands (soft rock, semi-arid, moderate height, `k` 2, full carve) score about equal to today's baseline and far better than SPIKE-80's authored class;
   - the derived site is chosen by drivers, not by today's elevation, so the arid class no longer depends on hand-picked ground (SPIKE-77 open point).
3. **The sand-sea rule needs sharpening:** blended at the edge of its window, the derived sand sea keeps `k` near 1 and overshoots. The authored sand-sea class remains the reference.
4. **Drivers before rules is a good fit for ADR 0076:** the same driver vector (plate history, rock, aridity, height) can feed the relief class, ground materials and, later, soil (below). The rule table is an authored asset, edited without code.
5. **The analytic rock and the climate are new drivers.**
   - The rock hardness is a function of plate history and basins plus a noise, a stand-in for a lithology layer.
   - The aridity index uses a stand-in PET.
   - Both are pure functions of the seed (CON-06), cheap at 0.25 degree (the global preview stays at seconds, SPIKE-77).

## A soil-depth driver from the same inputs (no build)
- Soil depth is the balance of production and removal.
  - Production: weathering of rock, faster on soft rock and in warm, wet climate. It falls exponentially with depth (the soil production function, Heimsath et al. 1997).
  - Removal: hillslope transport, which grows with slope, and incision.
- From the drivers above it needs:
  - the rock hardness `H` (production rate);
  - the climate `P`, `T` (production rate, and runoff);
  - and from the bake (ADR 0088's erosion layer): the local slope after thermal, the carve depth (removal) and the deposition sweep (alluvial soil in valleys and basins).
- A steady-state form `h = h0 ln(P0 (H, P, T) / E (slope, carve))` per cell of the bake's resolution, plus the deposited thickness, is a post-bake field per tile. It needs no new global stage: the drivers here and the erosion layer are enough.
- It replaces SPIKE-30's soil stand-in (ADR 0076 open points) and feeds the material and plant rules.

## Open points
- The site evidence is SPIKE-80's stand-in (the delta on today's field), not a re-bake.
- The class `k` of badlands (2) and sand sea (0.35) are set a priori; the badlands' landform classes (L1 0.60-0.63) show the small-scale shape is not yet right.
- PET is a stand-in (300 + 45 T mm); a radiation-based PET needs insolation, which the drivers do not have at 0.25 degree.
- The rock hardness has no data behind it: no plate ages, no sediment budget. A sediment-routing driver (where the carve's material goes) would make "basin" physical.
- One seed; the rule table's curves are first guesses.

## Visuals
- `images/derived-classes-and-drivers.png`: derived classes (left) and the drivers (right), equirectangular, 0.25 degree shown at 0.5.
- `images/N43W102-stills.jpg`, `images/N31W004-stills.jpg`: still sheets for the Badlands and Erg Chebbi.

## Handover
- Climate: on branch `spike/m9-plains`, `S81_CLIMATE=target/s81/climate.f32 target/release/examples/terrain_score.exe` (about 5 s).
- Classes: `python -I proto/classdrivers.py.txt target/s69/map13.f32 target/s81 <real-stats.json> target/s81/climate.f32 <SPIKE-80 combined.py>` (about 30 s; real stats in `spikes/m9-combined/results/raw/real-stats.json`). It writes `s81-fields.f32` (16 MB, kept out of git) and `classdrivers.json`.
- Sites: write `sites-derived.toml` from `classdrivers.json`, run `proto/run.sh.txt`, then SPIKE-80's `summarise.py`.
