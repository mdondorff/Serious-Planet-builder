# SPIKE-71: bake time margin

Opened: 2026-10-31
Question: Can the two-pass bake of ADR 0088 (SPIKE-70's hybrid) be brought to 7 h or less for the planet? Three parts:
- does the 1 m land floor remove the Highlands seam;
- does an active-cell thermal kernel give the full Jacobi's result in less time;
- do point samples from the field at the face pass's grid replace the block means of 100 m samples?
Why it matters: SPIKE-70 estimated 7.5-10 h, at M9 criterion 7's 10 h limit.
Time box: 2.5 h (opus-planner, 2026-10-31); used about 1 h 40 min, with 40 min of machine time.
Success criteria: the seam at the four sites against the off-edge step; thermal time per tile and the largest difference to the full Jacobi; the coarse sampling's time and relief difference; a new planet estimate.
Code location: offline crate `proto/s71bake.rs.txt` (from SPIKE-70's). The coarse dumps and the octave cap (`erosion::spike71_set_octave_cap`) are on branch `spike/m9-relief-source` (commit d92714d7). Spike code, never merged.
Closed: 2026-10-31

## Method
- **Windows and reference:** SPIKE-70's four 6,656^2 windows (x6 base) and its joint bake. Scoring as SPIKE-70 (`proto/score.py.txt`): ratios to the joint, and the seam as the edge step p99 against the off-edge p99.
- **Hybrid:**
  - the face pass at 8 x 8 (814 m) on the block means;
  - per tile, the local carve capped at 1 km^2 and thermal (33 degrees, 100 passes);
  - the 1 m land floor: the face change's upsampled cut never takes a land cell below 1 m (`hybrid1floor`).
- **Active-cell thermal:** a pass evaluates only the cells that changed in the pass before and their eight neighbours. A cell whose neighbourhood did not change keeps its zero update, so the result must equal the full Jacobi bit for bit.
  - Timed on tile 0 of each site (3,584^2) after the local carve: full Jacobi at 4 threads, active at 4 and at 1 thread.
- **Coarse sampling:** each 814 m cell sampled once at its centre, with all 12 crease octaves (`point12`) and with the octaves capped at 5 (wavelengths of 1.6 km and up, `oct5`).
  - Amplified on the coarse grid (x6 about the 20 km mean).
  - The face pass is run on it in place of the block means; the rest as `hybrid1floor`.
- **Runs:** BelowNormal under the heavy lock, CPU 3-5 % before (`raw/run.log`).

## Results
### Thermal on active cells (tile 0, 3,584^2, 100 passes)
| Site | Full Jacobi, 4 threads | Active, 4 threads | Active, 1 thread | Evaluations per cell (full 100) | Max difference |
|---|---|---|---|---|---|
| Black Forest | 21.2 s | 1.6 s | 3.4 s | 3.5 | 0 m |
| Highlands | 21.3 s | 2.7 s | 7.1 s | 5.3 | 0 m |
| Valley and Ridge | 20.7 s | 6.2 s | 12.3 s | 11.5 | 0 m |
| Alps | 22.8 s | 16.6 s | 31.8 s | 25.2 | 0 m |
- 1 and 4 threads give identical bits.
- The local carve before it: 13.5-16.4 s per tile, one thread.

### Coarse sampling (814 m, 832^2 cells, 4 threads)
| Site | 100 m samples for block means (6,656^2) | point12 | oct5 | point12 against block means: mean, \|d\| p50 / p99 | oct5: mean, \|d\| p50 / p99 |
|---|---|---|---|---|---|
| Alps | 32.8 s | 0.71 s | 0.62 s | 0.1, 11.7 / 182 m | 3.7, 10.7 / 178 m |
| Black Forest | 32.6 s | 0.67 s | 0.60 s | 0.0, 6.9 / 88 m | 2.5, 6.4 / 85 m |
| Highlands | 32.8 s | 0.71 s | 0.64 s | 0.1, 9.3 / 89 m | 3.6, 8.6 / 84 m |
| Valley and Ridge | 33.4 s | 0.72 s | 0.64 s | 0.1, 8.4 / 53 m | 2.9, 7.7 / 48 m |
- Point sampling is about 47x cheaper than the 64 samples per cell of block means.

### Relief and seams
Ratios to the joint are relief1 / relief5 / slope p50; seam is the edge p99 against the off-edge p99 (m):
| Site | hybrid1 (SPIKE-70) | hybrid1floor | hybrid1floor-point12 | hybrid1floor-oct5 |
|---|---|---|---|---|
| Black Forest | 0.98 / 1.00 / 1.07; 57 vs 59 | 0.96 / 0.93 / 1.06; 57 vs 58 | 0.93 / 0.90 / 1.04; 55 vs 58 | 0.94 / 0.90 / 1.04; 58 vs 57 |
| Highlands | 1.03 / 1.03 / 1.10; 99 vs 67 | 1.00 / 0.93 / 1.08; 99 vs 67 | 0.98 / 0.91 / 1.07; 93 vs 65 | 0.99 / 0.92 / 1.08; 79 vs 64 |
| Valley and Ridge | 0.95 / 0.95 / 1.07; 50 vs 52 | 0.93 / 0.89 / 1.05; 50 vs 52 | 0.91 / 0.87 / 1.03; 49 vs 52 | 0.92 / 0.87 / 1.04; 50 vs 52 |
| Alps | 1.03 / 0.97 / 1.11; 76 vs 76 | 1.02 / 0.93 / 1.10; 76 vs 76 | 0.99 / 0.92 / 1.09; 75 vs 74 | 0.99 / 0.92 / 1.09; 75 vs 75 |

## Planet estimate (from these timings, not a run)
| Pass | Work | Estimate |
|---|---|---|
| 1. Face sampling | 6 x 12,288^2 = 906 M point samples at 814 m (oct5 or point12), 4 threads; 0.6-0.7 s per 0.69 M | about 15 min |
| 1. Face carve | 6 faces of 151 M cells, about 3 min and 5.5 GB each (SPIKE-70), two at a time | about 10 min |
| 2. Per land tile | 100 m sampling 12-20 s (SPIKE-61, 4 tiles in parallel) + local carve 13.5-16.4 s + active thermal 3.4-31.8 s (one thread) + deposition and writing about 2 s | 31-70 s; about 44 s weighted (a quarter of land mountains, SPIKE-69) |
| 2. 2,140 land tiles, 4 at a time | 2,140 x 44 s / 4 | about 6.5 h |
| **Planet** | | **about 6-7.5 h (central 6.9 h)** |
- Against SPIKE-70's 7.5-10 h:
  - the second sampling pass is gone (point samples at 814 m: about 2 h saved);
  - thermal is 7-30 % of the full Jacobi's cost;
  - the central estimate meets the 7 h target, with no margin at the top of the range.

## Findings
1. **Active-cell thermal is exact and 1.4-13x faster** (max difference 0 m, thread-count independent). Mountains keep more cells active: Alps 25 evaluations per cell against 3.5 at the Black Forest.
2. **Point samples at 814 m replace the block means** at 1/47 of the cost. Relief changes by 1-3 % against the block-mean hybrid, and the seams are unchanged. Capping the octaves at 5 adds a +3 m mean offset and no clear benefit, so point12 is enough.
3. **The 1 m land floor does not fix the Highlands seam** (99 against 67 m) and lowers 5 km relief by 4-7 %. Below-sea valleys were not its cause. Of the variants tried, the seam is smallest with oct5 (79 against 64 m); its cause is still open.
4. **Planet estimate about 6-7.5 h (central 6.9 h):** the 7 h target is met at the centre, with no margin at the top.

## Open points
- The Highlands seam: a coastal window (33 % sea). The next suspects are sea cells as base level inside tile windows, and the local carve near the coast. It is R4 PR 5's measurement, and the 64-cell blend stays the fallback for the step.
- The floor is not recommended: it costs relief and fixes nothing measured.
- The estimate's tile mix (a quarter mountains) is from SPIKE-69's classes; 4 tiles in parallel as SPIKE-61. Concurrency effects on the local carve and thermal are not measured.
- Deposition is still unmeasured (estimated at 1 s).

## Visuals
- No new images: the looks of `hybrid1` are SPIKE-70's `images/blackforest-hybrid.jpg` and `images/highlands-hybrid.jpg`. The variants here differ from it by 1-7 % in relief.

## Handover
- **Dumps:** on branch `spike/m9-relief-source`, `RELIEF_COARSE=8 RELIEF_OCTAVES=5` with `relief_window_dump` (`proto/run.sh.txt`).
- **Run:** `s71bake <dump dir> <SPIKE-70 out dir> <sites>`; it reads SPIKE-70's `<site>-base.f32` and `-joint.f32`.
- **Score:** `python -I proto/score.py.txt <out dir> <sites> joint,hybrid1,hybrid1floor,hybrid1floor-point12,hybrid1floor-oct5 real.json out.json`.
