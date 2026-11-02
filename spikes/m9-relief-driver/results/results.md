# SPIKE-69: the relief-driver map (where k comes from)

Opened: 2026-10-31
Question: From what does the generator derive the per-region relief factor `k` of ADR 0088? Today's landform driver, as a linear weight, barely acts (SPIKE-66).
Why it matters: ADR 0088's open input; M9 R7 tunes against it. Owner's addendum (inbox #8): can it be previewed before the bake, and can a person paint it?
Time box: 3 h (opus-planner, 2026-10-31; Opus); used about 2 h 15 min, with 3 min of machine time.
Success criteria: at least three candidate sources, scored globally against Earth's land shares per relief class and locally at SPIKE-65's four sites; continuity and the coast taper checked; a recommended source with its curve; maps; risks.
Code location: branch `spike/m9-relief-driver` (pushed, no PR; commit 0c609792; spike code, never merged). It holds the ignored test `relief_driver_map` (`proto/relief_driver_map.rs.txt`) and `global::spike69_parts`; the analysis is `proto/analyse.py.txt`.
Closed: 2026-10-31

## Method
- **Map:** the generator's inputs on a 0.5-degree grid (720 x 360), seed 1, store `global16384` (read only; `raw/map.log`). Per land pixel:
  - the structure terms of `global.rs`: continent, range zone `zone`, inland ramp `inland`, ridge, hill band;
  - the bake elevation `e`;
  - the bake's elevation spread (std of 9 x 9 samples) over +-50 km and +-200 km;
  - the field's relief (max - min of the eroded field's heights): over 5 km (11 x 11 at 500 m), and within a 7 km radius (heights 1 km apart).
- **Sites:** SPIKE-65's four (Alps, Black Forest, Highlands, Valley and Ridge), with the same values at their centre.
- **Relief classes:** the relief within a 7 km radius after `k`, `R = k R14`; mountains also at 2,500 m and above:
  | Class | R |
  |---|---|
  | plains | under 100 m |
  | hills | 100 to 300 m |
  | low mountains | 300 to 1,000 m |
  | high mountains | 1,000 m and more |
  - The 300 m mountain bound follows the local-elevation-range criterion of Kapos et al. (2000). The definition was not read here (from memory), so it is a working bound, not a citation.
- **Earth's land shares** (Antarctica and Greenland excluded):
  | Group | Share | Source |
  |---|---|---|
  | mountains | 25 % | Meybeck, Green and Vörösmarty (2001), abstract and Viviroli et al. (2007); Sayre et al. (2018) give 30.6 % with a different definition |
  | mountains and higher land (hills, mid-altitude plains, platforms) | 39 % | Viviroli et al. (2007) |
  | targets used | plains 61 %, hills 14 %, mountains 25 % | the hills share is the 39 % minus 25 %, an upper bound |
  - The papers' class tables were not readable: two hosts failed the fetcher's certificate check and one asked for a CAPTCHA. Only the abstract figures are used.
- **Polar land** (|latitude| 60 degrees and more) is left out of the shares and the fit, as Earth's figures leave out Antarctica.
- **Candidate sources:** each gives a score per point. One monotone curve per source maps the score to `k`:
  - `k` is 1 up to the 50 % land quantile of the score, rising linearly in the quantile to `kh` at 66 %, `km` at 82 % and `kM` at 100 %;
  - the coast taper multiplies `k - 1` (smoothstep from 0 at the coast to 1 at 400 m, ADR 0088);
  - the knots are fitted on a grid so the class shares match Earth's.
  | Source | Score |
  |---|---|
  | landform today | the landform driver `L = zone x inland` (GEN-064, GEN-062's lattice) |
  | (a) structure | `zone (0.7 + 0.3 ridge)`: the range zone without the inland ramp, ridges weighted |
  | (c) the bake's long-wave relief | the 50 km elevation spread |
  | (b) a relief-class layer | `0.55 u + 0.45 zone`, with `u` a new low-frequency fBm on the direction (a stand-in for a new generator layer that a person could paint) |
- **Needed `k` at a site:** real 5 km relief over the generator's (SPIKE-65 windows' p50):
  | Site | Needed k |
  |---|---|
  | Alps | 6.9 |
  | Black Forest | 3.6 |
  | Highlands | 6.8 |
  | Valley and Ridge | 5.6 |
- **Continuity:** `|dk|` between neighbouring land pixels (0.5 degree, about 55 km), p99 and max.

## Results
The generator's relief today is low everywhere on land: R14 p10/p50/p90/p99 is 30 / 65 / 155 / 715 m. Without `k`, 81 % of land is plains and 4 % mountains.

Fitted curves and scores:
| Source | k at knots (50 / 66 / 82 / 100 %) | Plains / hills / low / high mountains | Fit error | dk p99 / max | k at Alps / Black Forest / Highlands / Valley and Ridge |
|---|---|---|---|---|---|
| none | 1 | 0.810 / 0.152 / 0.037 / 0.001 | - | 0 | 1 / 1 / 1 / 1 |
| landform today, linear `1 + 5 L` (SPIKE-66) | - | 0.720 / 0.151 / 0.085 / 0.044 | - | 0.51 / 1.18 | 2.2 / 1.9 / 1.5 / 1.7 |
| **landform today, fitted curve** | 1 / 4.5 / 4.5 / 5.6 | **0.626 / 0.124 / 0.189 / 0.061** | 0.032 | 1.6 / 3.5 | **4.6 / 4.5 / 3.9 / 4.3** |
| (a) structure | 1 / 7 / 10 / 15 | 0.645 / 0.105 / 0.106 / 0.144 | 0.071 | 3.3 / 13.5 | 12.0 / 8.2 / 8.7 / 9.4 |
| (c) bake spread 50 km | 1 / 2.5 / 10.5 / 15.8 | 0.609 / 0.142 / 0.095 / 0.154 | 0.005 | 5.2 / 11.8 | 13.6 / 11.5 / 9.8 / 10.4 |
| (b) relief-class layer | 1 / 5 / 13 / 19.5 | 0.611 / 0.139 / 0.107 / 0.142 | 0.002 | 4.3 / 18.2 | 16.7 / 3.1 / 11.2 / 12.7 |
| needed (sites) | | Earth 0.61 / 0.14 / 0.25 (mountains) | | | 6.9 / 3.6 / 6.8 / 5.6 |

The landform curve in values of `L` (the land quantiles of `L` outside the poles):
| L | k (before the coast taper) |
|---|---|
| 0 (49 % of land) | 1 |
| 0.028 (66 %) | 4.5 |
| 0.18 (82 %) | 4.5 |
| 1 | 5.6 |
- So any range-zone presence means about x4.5, rising to x5.6 at the cores.

**What the bake keeps** (SPIKE-65's windows, `relief5` after the bake over before):
| Site | amp4 | amp4 + carve + thermal | amp6 | amp6 + carve + thermal |
|---|---|---|---|---|
| Alps | 3.19 | 3.16 | 4.71 | 3.99 |
| Black Forest | 3.56 | 3.02 | 5.25 | 3.99 |
| Highlands | 3.26 | 3.35 | 4.76 | 4.28 |
| Valley and Ridge | 3.57 | 3.10 | 5.25 | 4.10 |
- The baked 5 km relief is about 0.8 k (amplitude alone, with the 20 km mean and the 1 m land floor) and 0.65-0.85 k after the carve and thermal.

### Maps (`images/`)
- `k-and-classes.png`: four rows, one per source (landform fitted, (a), (c), (b)). Left: `k` (1 dark to 12 yellow); right: the classes (green plains, tan hills, brown low mountains, white high mountains). Ocean and polar land are dark. Red circles mark the four sites.
- `inputs.png`: the generator's inputs over land. Top row: range zone, inland ramp, zone x ridge. Bottom row: 50 km spread, 7 km relief.
- Landform fitted: broad belts with k 4.5, plains elsewhere; mountain belts where the generator already has its ranges. No visible boundary at 0.5 degree.
- (c) follows the bake's own features as streaks and curls.
- (a) and (b) put k 10-20 over large flat interiors to reach the mountain share, amplifying small relief into uniform rough ground.

## Findings
1. **The landform driver is the right source; SPIKE-66's linear weight was the wrong curve.**
   - L is 0 on 49 % of land and small (0.09-0.18) at the hill sites, which rank in its top third.
   - A curve calibrated by land quantiles (k 1 at L 0, 4.5 by L 0.028, 5.6 at L 1) gives Earth's shares within 3 points (63 / 12 / 25 %).
   - Site k is 3.9-4.6 against 3.6-6.9 needed.
2. **The other sources match the shares only by over-amplifying.** Their score does not follow the generator's existing relief, so they need k 8-17 at the sites (2-3x too much) and k up to 20 on flat interiors. (c) also draws streaks from the bake's own features.
3. **The share target and the site target agree only for a source whose belts hold the existing relief.** The landform's belts do, because the generator's ranges and hill band are built from the same zone and inland terms.
4. **Continuity:** the landform curve changes `k` by at most 3.5 per 55 km (p99 1.6). With the L lattice at about 39 km and the coast taper at 400 m, no boundary shows.
5. **The Alps need more than the curve gives.** They need 6.9 and get 4.6, and the carve keeps 0.65-0.85. So the top knot (or a ridge term for `kM`) is R7's tuning. This is SPIKE-66's "about x7.5 for 0.8 of real".
6. **Pre-bake preview:**
   - L is a lattice of 257^2 points per face, about 0.5 s for the planet (GEN-062). k and the classes follow per point, so a whole-planet coarse preview at 4-16 km takes seconds.
   - It predicts the baked 5 km relief as about 0.75 k times today's relief (four sites, +-0.1 k).
   - At the four sites the preview (curve k times R14) classes the Alps as high mountains and the other three as low mountains.
   - SPIKE-65's baked amp6 windows (R14 taken as 1.55 x the 5 km relief, the map's median ratio) give low mountains at all four. The Alps differ: the bake keeps less relief there.
7. **Authorable:**
   - A person can paint L (or `k`) at continent scale as an override of the lattice: a soft brush blended into the samples.
   - The quantile curve keeps the global shares and k stays continuous, because the lattice is bilinear.
   - The bake honours it: `k` is a baked channel computed from the lattice at bake time (ADR 0088 decision 1), and its key covers the override.
   - (b) is the most natural layer to paint, but as built here it over-amplifies. The recommendation is to make L itself the paintable layer.

## Recommendation for the generator spec
- `k = 1 + taper(e) (g(L) - 1)`, with `L` GEN-062's landform lattice, `taper` the smoothstep from the coast to 400 m, and `g` piecewise linear in `L` through the knots in the table above. The knot values live in the spec; R7 tunes them, starting with the top knot for the Alps.
- The curve's knots are stated in L, not as quantiles, so the result does not depend on a global statistic at runtime. A tier-A test checks the land shares of a small bake against the targets.
- An override layer for L (paint) keyed into the bake's key: a later change, not M9 R4.

## Risks
- The class bounds and Earth targets are approximations: the published tables were unreadable here; mountains at 25-31 % depending on definition.
- k by structure alone gives one relief character per belt (about x4.5). Within-belt variety still comes from the generator's existing relief; badlands or plateaus with high k but low relief are not modelled.
- High mountains are 6 % against an unknown Earth share; the Alps need a higher top knot.
- Polar land was left out; the generator's polar continent gets the same curve.
- Seed 1 only. Another seed has a different L distribution; the knots in L hold the shape but not exactly the shares.

## Visuals
- `images/k-and-classes.png`: k and the classes per source on the 0.5-degree map, sites marked.
- `images/inputs.png`: the generator's inputs (zone, inland, zone x ridge, 50 km spread, 7 km relief).

## Handover
- **Map:** on branch `spike/m9-relief-driver`, `proto/map.sh.txt` (the ignored test `relief_driver_map`; output `target/s69/map13.f32`, 720 x 360 x 13 f32).
- **Analysis:** `python -I proto/analyse.py.txt <map13.f32> raw/sites13.txt <out>`; prints the table, writes `s69.json` and the images (about 2.5 min).
