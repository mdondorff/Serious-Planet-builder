# SPIKE-70: global routing for the carve

Opened: 2026-10-31
Question: Which tiling of ADR 0088's bake keeps the relief of a joint bake without the core-edge seams SPIKE-68 found:
- (c) the carve on a coarse grid per face, with only thermal (and deposition) per tile;
- (a) per-tile carves with the large rivers forced to a global routing?
And what does the bake then cost?
Why it matters: ADR 0088 decision 4 is open (SPIKE-68); the m9-bake change's PR plan depends on the tiling.
Time box: 3 h (opus-planner, 2026-10-31; Opus until Sunday 08:00); used about 2 h 50 min, with 1 h 25 min of machine time.
Success criteria:
- the relief that survives (SPIKE-65's measures at its four sites, against SPIKE-68's full-resolution joint bake);
- the step across tile edges;
- cost and memory for a whole face;
- SPIKE-61's per-tile timing redone with the inflow on interior cells.
Code location: offline crate `proto/s70carve.rs.txt` (from SPIKE-68's), the two extra windows dumped on branch `spike/m9-relief-source`, and SPIKE-61's fix on branch `spike/m9-bake-prerun` (commit after 51b0a191, pushed). Spike code, never merged.
Closed: 2026-10-31

## Method
- **Windows:** SPIKE-68's 6,656^2 windows at about 100 m: Alps, Black Forest, plus Highlands and Valley and Ridge (`raw/run.log`).
  - Each holds 2 x 2 tile cores of 3,072^2 with 256-cell halos.
  - Each is amplified x6 about the 20 km mean, as SPIKE-68.
- **Bake:** ADR 0088's order (carve: U/K 20, K dt 0.05, 20 steps, re-route every 5th, breach; thermal 33 degrees, 100 passes).
- **Variants:**
  | Id | What |
  |---|---|
  | joint | the whole window baked at once (reference, SPIKE-68) |
  | s68tiles | SPIKE-68's plain tiles: per-tile carve and thermal (Alps and Black Forest only) |
  | coarse400, coarse800 | (c): the carve on 4 x 4 or 8 x 8 block means (407 m, 814 m) over the window; its change upsampled bilinearly to 100 m and added to the base; thermal per tile (256-cell halo); cores mosaicked |
  | forced10, forced100 | (a): per-tile carve and thermal; cells whose drainage area in the window's routing of the base is at least 10 or 100 km^2 take that routing's receiver and area; inflow at interior cells |
  | hybrid10, hybrid1 | coarse800, then per tile a 100 m carve whose drainage area is capped at 10 or 1 km^2 (only small catchments incise), then thermal |
- **Scoring** (`proto/score.py.txt`, `raw/score.log`) on the central 3,072^2, round the four-tile corner:
  - SPIKE-65's measures: slope p50/p90/p99, 1 km and 5 km relief p50, flat share, slopes over 20 and 33 degrees;
  - each as a ratio to the joint bake, and to real GLO-30 (SPIKE-65's `real.json`);
  - **seam:** the surface's own height step across the core edges against its steps 150 and 300 cells off the edge, p50 and p99. Equal values mean no seam. SPIKE-68's measure (the step against the joint) mixes a different carve with a seam, so this one replaces it.
- **SPIKE-61 redo:** run d of SPIKE-61 (face 3, every 4th tile, 33 land tiles, 4 in parallel, 10 steps) with the inflow placed one cell inside the window (`raw/d-face3-s4-inflow.txt`).
- **Runs:** at BelowNormal under the heavy lock, CPU 0-9 % before each step.

## Results
### Relief and seams
Ratios to the joint bake are relief1 / relief5 / slope p50. Seam is the edge step p99 against the off-edge p99 (m).
| Site | Variant | Ratios to joint | Flat (joint) | Seam p99 vs off-edge |
|---|---|---|---|---|
| Black Forest | s68tiles | 0.98 / 0.98 / 0.99 | 11.6 % (11.4) | 134.7 vs 63.7 |
| Black Forest | coarse400 | 0.91 / 1.02 / 0.89 | 4.8 % | 43.0 vs 43.5 |
| Black Forest | coarse800 | 0.94 / 1.09 / 0.86 | 5.6 % | 40.2 vs 40.6 |
| Black Forest | forced10 | 1.00 / 1.00 / 1.00 | 11.3 % | 83.6 vs 64.1 |
| Black Forest | forced100 | 1.00 / 1.00 / 1.00 | 11.4 % | 124.0 vs 64.2 |
| Highlands | coarse400 | 0.94 / 1.03 / 0.91 | 8.0 % (12.4) | 57.8 vs 63.7 |
| Highlands | coarse800 | 0.95 / 1.09 / 0.85 | 9.9 % | 51.8 vs 58.3 |
| Highlands | forced10 | 1.01 / 1.00 / 1.01 | 12.3 % | 76.4 vs 70.2 |
| Highlands | forced100 | 1.00 / 1.00 / 1.00 | 12.4 % | 107.7 vs 70.1 |
| Valley and Ridge | coarse400 | 0.90 / 0.98 / 0.88 | 7.5 % (12.7) | 34.9 vs 35.6 |
| Valley and Ridge | coarse800 | 0.92 / 1.02 / 0.84 | 8.9 % | 33.8 vs 35.3 |
| Valley and Ridge | forced10 | 1.00 / 1.00 / 1.00 | 12.6 % | 61.3 vs 60.4 |
| Valley and Ridge | forced100 | 1.00 / 1.00 / 1.00 | 12.6 % | 61.6 vs 60.4 |
| Alps | s68tiles | 0.98 / 0.97 / 0.98 | 14.2 % (14.0) | 174.9 vs 80.2 |
| Alps | coarse400 | 1.06 / 1.06 / 1.05 | 2.8 % | 78.6 vs 78.9 |
| Alps | coarse800 | 1.12 / 1.10 / 1.07 | 3.4 % | 77.5 vs 80.1 |
| Alps | forced10 | 1.01 / 1.01 / 1.01 | 13.8 % | 113.3 vs 80.7 |
| Alps | forced100 | 1.00 / 1.00 / 1.00 | 13.9 % | 108.0 vs 80.6 |
- Against real (GLO-30), the joint bake is 0.55-1.27 (relief1) and 0.61-1.32 (relief5) across the sites. coarse800 is 0.62-1.20 and 0.67-1.44: the same band.
- Slope p90 falls with the coarse carve: for example Black Forest 16.2 against the joint's 22.4 degrees. The steep valley walls of the 100 m carve are missing.

### Look (`images/`)
- `blackforest-variants.jpg`, `alps-variants.jpg`: hillshade of 104 km round the four-tile corner. Columns: joint, coarse800, forced10, and |coarse800 - joint| (0 to 50 m).
- coarse800 keeps the large valleys and the relief but has no fine channel network: the amplified base's texture shows where the joint cuts 100 m channels and floors.
- forced10 looks like the joint except near the edges.

### Hybrid (coarse800, then a per-tile 100 m carve with the drainage area capped)
Ratios to the joint are relief1 / relief5 / slope p50; seam is the edge p99 against the off-edge p99 (m):
| Site | Variant | Ratios to joint | Slope p90 (joint) | Flat | Seam p99 vs off-edge |
|---|---|---|---|---|---|
| Black Forest | hybrid10 | 1.12 / 1.06 / 1.21 | 21.7 (22.4) | 3.7 % | 61.4 vs 62.9 |
| Black Forest | hybrid1 | 0.98 / 1.00 / 1.07 | 19.7 | 5.1 % | 57.3 vs 58.5 |
| Highlands | hybrid10 | 1.19 / 1.12 / 1.27 | 22.7 (22.4) | 5.2 % | 98.9 vs 69.4 |
| Highlands | hybrid1 | 1.03 / 1.03 / 1.10 | 20.3 | 6.8 % | 99.0 vs 66.8 |
| Valley and Ridge | hybrid10 | 1.07 / 0.98 / 1.19 | 17.4 (18.3) | 4.9 % | 54.8 vs 56.4 |
| Valley and Ridge | hybrid1 | 0.95 / 0.95 / 1.07 | 15.8 | 6.2 % | 49.9 vs 52.2 |
| Alps | hybrid10 | 1.18 / 1.03 / 1.30 | 31.4 (32.5) | 4.7 % | 77.6 vs 77.0 |
| Alps | hybrid1 | 1.03 / 0.97 / 1.11 | 30.5 | 7.7 % | 76.2 vs 76.0 |
- hybrid1 is closest to the joint bake: relief 0.95-1.07, slope p50 1.07-1.11, and most of the slope p90 back (15.8-30.5 against 18.3-32.5). Flats are 5-8 % (real 6-9 %).
- No seam at three sites. The Highlands keeps one (p99 99 against 67 m) with either cap.
- **Below-sea valleys:** the coarse carve's upsampled change pushes low valley floors below sea level (blue dots along the rivers in `images/highlands-hybrid.jpg`). Those cells become outlets in the per-tile routing; this is the likely cause of the Highlands seam.
  - Fix (not run): clamp the layered height of a land cell to at least 1 m, as the amplitude step does.
- Cost: 150-155 s for four tiles (carve and thermal), as a plain tile.

### Cost
| Item | Measured (window) | Per face (estimate) |
|---|---|---|
| Joint bake, 6,656^2 (44 M cells), one thread routing, 4-thread thermal | 141-147 s | not feasible at 100 m (9.7 G cells) |
| One route of the window | 8.7-12.7 s | - |
| Carve at 814 m, 832^2 (0.69 M cells) | 0.5-0.6 s | 12,288^2 = 151 M cells: about 3 min, about 5.5 GB (36 B a cell) |
| Carve at 407 m, 1,664^2 (2.8 M cells) | 2.4-2.8 s | 24,576^2 = 604 M cells: about 15 min, about 22 GB: over the 16 GB machine without out-of-core routing |
| Thermal per tile (3,584^2, 100 passes, full Jacobi, 4 threads) | 21-23 s a tile | 2,140 land tiles: about 13 h at 4 threads; an active-cell kernel (SPIKE-64) is needed |
| Forced-river tile bake (carve and thermal) | 37-43 s a tile | as SPIKE-61, plus seams |

SPIKE-61 run d redone (33 land tiles, p50):
| Inflow | Sample | Route | Carve | Total p50 / p90 | Max cut p50 | Channel cut p50 |
|---|---|---|---|---|---|---|
| on edge cells (SPIKE-61) | 11.8 s | 33.2 s | 4.7 s | 53.4 / 86.1 s | 32.1 m | 0.56 m |
| on interior cells | 12.0 s | 34.9 s | 5.4 s | 57.6 / 84.2 s | 32.1 m | 0.56 m |
- SPIKE-61's timings stand within 8 %. Its cut was capped by local relief, so the inflow changed no cut statistic.

## Findings
1. **(c) The carve on a coarse grid per face removes the seams entirely.** Edge steps equal off-edge steps at all four sites. Thermal is local (100 passes, under 100 cells of reach), so per-tile thermal leaves no seam.
2. **(c) keeps the relief:** 1 and 5 km relief at 0.90-1.12 of the joint bake, slope p50 at 0.84-1.07. It leaves fewer flats (3-10 % against 11-14 %; real 6-9 %). It loses the 100 m channel network and the steep valley walls (slope p90 down by about a quarter).
3. **(a) Forced rivers reproduce the joint's relief exactly but not seam-free.**
   - At 10 km^2: edge p99 61-113 m against 60-81 m off-edge.
   - At 100 km^2: 62-124 m against 60-81 m.
   - The small catchments that cross a tile edge still differ, and the forcing needs a global 100 m routing that no face fits in memory.
4. **800 m is the coarse grid that fits:** about 3 min and 5.5 GB per face. 400 m needs about 22 GB.
5. **Thermal is now the bake's cost:** about 13 h for the planet as a full Jacobi at 4 threads. An active-cell kernel or fewer passes is required before R4's full bake (SPIKE-64's open point).
6. **SPIKE-61's inflow fix changes its timing by 8 %;** its per-tile estimates stand.
7. **The hybrid (800 m face carve, then a local 100 m carve of catchments up to 1 km^2) is the best tiling.** It keeps the joint bake's relief (0.95-1.07), most of its steep slopes and realistic flats. It is seam-free at three of four sites; the fourth needs the below-sea clamp, to be re-measured in R4.

## Recommendation
- ADR 0088 decision 4: the erosion layer in two passes:
  1. **per face at about 800 m:** block means of the base (point 2), the transient carve with breach, re-routing every 5th step;
  2. **per tile at 100 m (256-cell halo, core stored):**
     - the base plus the face carve's change (bilinear), land held at 1 m or more;
     - a local carve with the drainage area capped at about 1 km^2;
     - thermal on active cells;
     - one deposition sweep.
- Faces do not route across cube edges (a known limit, as the coupled run). A cube-edge seam is R4's measurement.
- R4's PR order changes: the face carve before the tile driver, and an active-cell thermal kernel with a timing gate.

## Open points
- Deposition was not part of any variant.
- The face carve's cube-edge seam and the 800 m sampling cost (block means need the 100 m samples, so pass 1 samples every tile once) are not measured.
- Thermal's cost with active cells is an estimate from SPIKE-64.
- Four windows of 680 km; one seed.

## Visuals
- `images/blackforest-variants.jpg`, `images/alps-variants.jpg`: joint / coarse800 / forced10 / |coarse800 - joint|.
- `images/blackforest-hybrid.jpg`, `images/highlands-hybrid.jpg`: joint / hybrid1 / coarse800 / |hybrid1 - joint|; blue cells are at or below sea level.

## Handover
- **Build:** the crate from `proto/s70carve.rs.txt` (as `src/main.rs`) and `proto/Cargo.toml.txt`.
- **Run:** `s70carve <dump dir> <out dir> <sites>` (all variants); then `S70_MODE=hybrid` (reads `<out>/<site>-base.f32` and `-joint.f32`). The drivers are `proto/run.sh.txt` and `proto/hybrid.sh.txt`.
- **Score:** `python -I proto/score.py.txt <out dir> <sites> <variants> real.json out.json`.
- The full-window results are in `SeriousPlanetData/spike-r3/relief70/` (about 5 GB; delete after R4's design is settled).
