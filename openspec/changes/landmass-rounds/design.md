## Context
- A3 section 6.6 and inbox #101: the world-build budget is 12 GB (ADR 0115 point 7, owner 2026-11-08); the partitioned route is the normal path for large landmasses; the resident route is an optimisation for a landmass that fits; the threshold is measured. The landmass of GEN-144 at the erosion grid has about 149e6 cells.
- The masked cube grid (GEN-144) holds its arrays over all `6 n^2` cells: 906e6 at 12,288 cells, about 80 GB. The measurement asks for a compact layout.

## Decisions
1. `Grid::compact_cube(n, radius, bits)`: the active cells only, `w` their number and `h` 1, the compact index of a cell its rank in the order of the cube ids. `ActiveIndex` holds the bitset, a table of the bits before each word (4 bytes a 64 cells) and the cube id of each cell (4 bytes a cell). Rank and neighbour lookups are a popcount and a cube neighbour (GEN-144).
2. Because the order is the cube id order, every visit by cell index (the ties of the flood, the order of the area, the sorts) is as on the masked grid: the solve gives the same bits (tier A on a sphere of 24 cells). At 3,072 cells the example's landmass has the same 9,308,154 land cells and 100,602 catchments as the printed output of `landmass_region` (GEN-144); that comparison is of printed numbers, not a test. The fine landmass at k = 4 is not compared with a full-sphere landmass.
3. The landmass at a refinement `k` of the 3,072-cell grid is found without sampling the whole sphere: from the 3,072-cell base cache (land connected to the region centre, two rings), the fine base is sampled only in the fine cells whose coarse parent is in that set, the fine land is the connected land above the sea there, its coast ring the outlets.
4. The partitioned route needs the rectangles of a face (`p % w` arithmetic) and does not run on a compact grid (`partitioned_route` refuses one). The partitioned rounds at landmass scale (briefing item 9, inbox #101) are therefore NOT measured here: only the resident route is. The owner's decision stands (the partitioned route is the normal path for large landmasses, the resident route an optimisation for those that fit); this change measures the threshold of the optimisation. The partitioned route on a compact grid, with the round's per-catchment arrays bounded, stays open.

## Measurements (`examples/landmass_scale`, release, 8 threads, 4 rounds of 5 steps, round rules, the cube metric, seed 14; Windows working set read by PowerShell)
| k (cells a face edge) | Land cells | Active cells | Catchments, largest | Peak, rounds | Bytes a cell | Time, 4 rounds |
|---|---|---|---|---|---|---|
| 1 (3,072) | 9,308,154 | 9,408,756 | 100,602, 389,051 | 764 MiB | 85 | 79 s |
| 2 (6,144) | 37,232,222 | 37,436,731 | 204,509, 1,558,326 | 2,198 MiB | 62 | 375 s |
| 4 (12,288, the erosion grid) | 148,929,871 | 149,341,566 | 411,695, 6,160,238 | **7,858 MiB** | **55** | 1,716 s |
- The peak is the process's, with the heights (f64), the outlets, the compact index and the routing and round arrays; the set-up peak is 3,641 MiB at k = 4 (the fine candidates and their heights, freed before the rounds).
- The largest catchment is about 4 % of the land at each k, so the in-flight copies of the catchments being carved (about 49 bytes a cell each, 8 at a time) are a stable share of the peak.
- Region land change at k = 4 (n 267,575): p5 -410.8 m, p50 -45.9 m, p95 8.6 m, max 18.9 m; the window runs of region-12288 gave p50 -45.5 and -45.8 m: the artificial border changed the region by under 0.5 m at p50.
- Sampling the base takes 807 s at k = 4 (152e6 fine candidates); the rounds take 1,716 s.
- **H1 on the same landmass at k = 2** (runoff of the orographic climate, weighted route, filled surface, `hydrology`): 128 s for the route and the filled surface, 9.4 s for H1; peak 3,121 MiB = **87 bytes a cell** (the peak of the rounds in that run was 2,190 MiB). **H1 was not run at k = 4**: at 149.3e6 cells the linear extrapolation is 13.0e9 bytes (12.1 GiB), above the budget of 12e9 bytes by about 1e9 bytes if the pass runs as the example runs it. An estimate is not a pass (A3 section 6.6).
  - Candidates, not built: the potential `Q` array (8 bytes a cell; only the conservation check reads it), the runoff and evaporation f32 arrays (8; computable on the fly), the order array of the cascade (4). About 20 bytes a cell together, an estimate.

## Threshold of the resident route
- Rule: the resident route is chosen when `cells x bytes a cell <= budget`, the budget 12e9 bytes (decimal).
- The rounds cost 55 bytes a cell at 149e6 cells (62 at 37e6, 85 at 9e6; the figure falls with size and assumes the largest catchment stays near 4 % of the land): 12e9 / 55 is about 218e6 cells. This is an extrapolation from three points.
- The H1 pass as written (87 bytes a cell, one point): 12e9 / 87 is about 138e6 cells, below this landmass.
- A landmass above the threshold needs the partitioned (non-resident) route of A3 section 6.6, which is not built on a compact grid.

## Not measured or not claimed
- The partitioned rounds at landmass scale, and the per-catchment arrays of a non-resident form.
- H1 at k = 4, and the whole build's peak (the base sampling, the rounds and H1 were measured in separate phases; G-SCALE counts the build's peak with inputs and drivers).
- The edit memo caches and the editor's memory.
- G-DET (one run, 8 threads, one machine, Windows) and the rest of G-SCALE (flat-area cost, the H0-split fallback, H0 at twice the radius).

## Earth-specific values (ADR 0108 point 4)
| Value | Earth (example) | Unit, source | Parameter |
|---|---|---|---|
| Base grid and refinement | 3,072 cells and k = 1, 2, 4 (12,288 cells, 814 m) | cells; the prototype | the example `COARSE`, `k` |
| Seed cell | face 1, (2090, 267) of the 3,072 grid | the centre of region 1 | the example `SEED_CELL` |
| Candidate rings | 2 coarse cells | calibration, not measured: the fine coast may lie up to one coarse cell from the coarse coast | the example |
| Rounds | 4 of 5 steps | the prototype | `CarveParams` |
| Radius, seed, sea level | 6,371,000 m, 14, 0 m | Earth state; world definition | the example |
| Round parameters | 5 steps a round, the carve defaults, epsilon 1 mm | calibration (GEN-100) | `CarveParams::default()` |
