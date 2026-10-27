

# SPIKE-33 results: landscape evolution over time (landscape-evolution)

Machine: Rasierklinge (RTX 3080 Ti Laptop, 20 threads), release build of a standalone CPU prototype, **laptop lid closed, no display attached; other sessions running** (all timings indicative, no GPU involved, no `cargo xtask perf`). Prototype: `proto/` (also `landscape-evolution-prototype.patch`), built on SPIKE-31's cube-sphere layout; numbers are in `raw/`.

## Verdict
- **Dynamic coupled evolution (A: route, update, re-route on every step) does not fit as a per-block stage.** Re-routing makes the result chaotic at a block border: at N = 30 steps and a 5 km halo (one coarse ring) a block differs from the truth (the same run over 20 km) by p50 / p90 / p99 / max 0.0 / 2.2 / 28 / 108 m, and no halo from 0.5 to 5 km gets p99 under 1 m (at N = 10, p99 is 10 m at 5 km). Feeding the block the exact border flux of the truth does not help (29 m p99), and freezing the routing makes it worse (102 m). The light cone is not N cells; it is the whole drainage. It also costs 3.2 s per 5 km block (20 m cells, 5 km halo) against 0.5 s for A-net.
- **A-net (the coarse coupled run fixes the river network; the fine steps run along that frozen network) fits all five engine rules and gives exact seams.** With the receivers a position-only rule, two neighbouring blocks (N = 30, 5 km halo) equal the truth with the same network in both 5 km cores to the printed precision (p99 0.00 m, max 0.0 m, |ln A| mean 0.000), so blend and hard join are identical and no seam exists. A block costs 0.4 to 0.5 s at 20 m cells and 30 ns per cell-step (dynamic A: 200 to 230 ns), 0.06 s per km2 at 4 m cells (49 km2 in 2.8 s, N = 30) against 0.37 s per km2 dynamic, 0.21 for droplets (C) and 0.010 for erosion noise (B).
- **The look is better than B and C where drainage is concerned** (`images/sheet_*.png`, below): A-net carves coherent dendritic networks that end in the coarse river; B reads as speckle with no drainage, C as rounded ridges with no networks. A-net's flaws are visible too (D8 rills line up with the grid axes, and the coarse path appears as a straight groove). **The look needs engine renders with shading and the material map before it can be compared with SPIKE-32's stills; this spike shows hillshades only.**
- **Recommendation:** a global coarse run once per world version (2048, about 70 to 100 steps, 4 to 5 min on 8 threads, 1.1 GB; outputs heights, A, receivers, U, K), then a regional **block stage on the fixed 5 km grid** that runs a capped N = 30 steps of implicit stream power along the frozen coarse network, plus diffusion and slumping, on a fixed-resolution raster (4 to 10 m); LOD parents are box-filtered from that one raster (a 20 m and a 40 m run of the same block differ by p50 19 m, p99 100 m). Open before M8 commits: the halo for A-net below 5 km (only 5 km was measured), the breach rule (19.7 % of channel cells still run uphill), and fine drainage that is not axis-aligned.

## Stage model this implies
| Item | Value |
|---|---|
| Global stage | coarse coupled run at 2048 (5 km cells): U from the global height, K from lithology, single steepest receiver, implicit stream power, dt 1e5 yr; stored: heights, A (m2), receivers; the network of cells with A over 100 km2 is the river network of every block |
| Block stage | fixed grid of 5 km blocks; fine raster 4 to 10 m; ring of fixed heights at the halo border; coarse flux injected at the ring (the coarse crossing, valley-weighted) |
| Fine steps | N = 30 (cap), each: implicit stream power along the frozen network, diffusion, slumping; no routing, no flood |
| Halo | 5 km measured exact; the local light cone of N = 30 steps is about 60 cells of diffusion and slumping, 0.24 km at 4 m (not measured as a halo sweep, an open item) |
| Determinism | block = pure function of the coarse state, block id and parameters; sequential inside a block, parallel across blocks |
| Seams | none in the cores (identical fields); a seam appears only where a block's own ring (heights fixed from the coarse state) meets the next block's core |

## Measurements
All error numbers are from the same site per table. Site 1 (dynamic A, tables 2 to 4): coarse 1024 grid, lat 51.706 lon 32.646, face 4; site 2 (A-net, look): coarse 2048 grid, lat 22.536 lon -8.943, face 0 (the best-relief inland coarse cell, 100 km or more from a face edge, chosen by the run; SPIKE-32's seed-1 site could not be reused because the world here is seed 42 and the fine initial state is the coarse height plus a ridged multifractal, 400 m amplitude). The model is deliberately simple: n = 1, m = 0.45, K = 2e-5 times lithology, U = 1e-3 m/yr per 1000 m of base height, 10 000-year fine step, a critical area of 2,500 m2, diffusion and slumping at 42 degrees; uplift is off in the fine run (it relaxes the detail onto the coarse state). The coarse state is not tuned to the engine's world.

### 1. Coarse global run (measurements 5; and the convergence curve)
Start at 0.6 times the base height; steps of 1e5 yr; mean |dz| per step over land:
| Step | 0 | 5 | 10 | 20 | 40 | 60 | 80 | 100 | 150 | 199 |
|---|---|---|---|---|---|---|---|---|---|---|
| mean abs dz (m) | 33.8 | 16.4 | 9.5 | 2.2 | 0.94 | 0.62 | 0.42 | 0.31 | 0.15 | 0.085 |
The mean change falls under 1 m after about 40 steps and under 0.3 m after 100; the maximum stays at 150 to 250 m (lakes filling and divides moving; it does not converge in this model).
| Grid | Time per step | Memory (peak working set) | Notes |
|---|---|---|---|
| 1024 (10 km) | 0.62 to 0.9 s, 8 threads | 0.29 GB | 100 steps about 75 s |
| 2048 (5 km) | 2.6 to 3.1 s, 8 threads; 4.1 to 4.8 s, 1 thread | 1.07 to 1.16 GB | base sampling 6.1 s (8 threads), 32.7 s (1 thread); 100 steps about 5 min |
| 4096 (2.4 km, extrapolated) | about 11 to 13 s, 8 threads | about 4.3 GB | 100 steps about 20 min; not run |

### 2. Light cone of dynamic A (measurement 2, `raw/sweep.txt`)
Truth: the whole 20 km lattice at 20 m, N steps; blocks of 5 km (four central blocks), halo H, error over the cores. Height error |dz| in m, mean |ln A| difference:
| N | H = 0 (the stand-in: coarse plus detail, no run) | H 0.5 km | 1 km | 2 km | 5 km |
|---|---|---|---|---|---|
| 10 | p50 4.9, p99 58 | p50 0.02, p90 6.2, p99 39, dlnA 0.48 | p99 36, dlnA 0.47 | p99 32, dlnA 0.35 | p50 0.00, p90 0.22, p99 10.3, max 80, dlnA 0.09 |
| 30 | p50 14, p99 130 | p99 89, dlnA 0.60 | p99 79, dlnA 0.60 | p99 64, dlnA 0.45 | p50 0.00, p90 2.2, p99 28, max 108, dlnA 0.15 |
| 100 | p50 61, p99 293 | p99 229, dlnA 0.72 | p99 211, dlnA 0.71 | p99 147, dlnA 0.53 | p50 0.35, p90 16.5, p99 62, max 193, dlnA 0.20 |
**The H(N) rule asked for does not exist for dynamic A:** no tested H gives p99 under 1 m at any N. At the largest H (5 km) the p90 is under 1 m only for N below about 10. p99 relative to the core's relief (about 600 m): 4.6 % at N = 30, H = 5 km.

Why (N = 30, `raw/oracle.txt`, `raw/variants.txt`):
| Variant (H = 5 km) | p50 | p90 | p99 | Share of p99 tail in channels |
|---|---|---|---|---|
| coarse border flux, dynamic routing | 0.00 | 2.19 | 28.3 | 29 % (channels are 6.1 % of cells) |
| oracle border flux from the truth, dynamic | 0.00 | 1.44 | 29.1 | 28 % |
| oracle ring heights from the truth (N = 30, H = 5 km, `oracle.txt`) | 0.00 | 1.92 | 29.0 | not measured |
| coarse flux, frozen routing (receivers and A from step 0) | 5.79 | 29.4 | 102 | 2 % |
| oracle flux, frozen routing | 5.77 | 29.2 | 102 | 2 % |
| truth with frozen routing against truth with dynamic routing | 6.03 | 32.5 | 109 | 2 % |
Neither the border heights nor the border flux is the cause; re-routing during the run moves divides and river courses, so a small border difference becomes a different drainage inside the block. The error sits on slopes (71 % of the tail) and is five times more likely than chance in channels (p99 in channels 55 m, on slopes 25 m).

### 3. Seams and the overlap blend (measurement 3; item asked by the planner)
Two neighbouring blocks (cores x 250..500 and 500..750 cells, rows 250..500, N = 30, H = 5 km, 20 m), the seam at x = 500. The blend weights each block by its distance to its own ring across the 10 km overlap (SPIKE-31). The "interior reference" is a row at x = 375, so it is a different place, not a seam-free copy.
| | Height step across the seam (m) p50 / p90 / p99 / max | Slope jump (1/m) p50 / p99 |
|---|---|---|
| dynamic A, hard join | 4.0 / 11.0 / 29 / 55 | 0.16 / 0.54 |
| dynamic A, overlap blend | 3.6 / 9.6 / 15 / 28 | 0.15 / 0.52 |
| dynamic A, interior reference | 2.5 / 7.3 / 12 / 15 | 0.08 / 0.71 |
| A-net, hard join (= blend: the blocks agree) | 2.7 / 8.6 / 19 / 22 | 0.12 / 0.56 |
| A-net, interior reference | 1.8 / 7.4 / 14 / 15 | 0.05 / 0.32 |
Blend zone (x 252..748, 500 cells):
| | The two blocks differ, p50 / p90 / p99 / max (m) | Channel cells (A over 50,000 m2) whose receiver is higher in the blended surface | Cells that are a channel in one block and under a fifth of that area in the other |
|---|---|---|---|
| dynamic A | 0.06 / 5.8 / 29 / 134 | 7.8 % | 35.5 % |
| A-net, as first built | 0.00 / 0.09 / 48 / 218 | 31.0 % | 2.9 % |
| A-net, with the breach rule | 0.00 / 0.07 / 65 / 218 | 19.7 % | 2.9 % |
The blocks of A-net agree everywhere except next to the other block's ring (p99 and max in the zone). **Correction:** the row count "N of M rows where a channel reaches the seam on one side only" (12 of 14 dynamic, 12 of 15 A-net) compares two adjacent cells, so a channel that crosses a cell boundary counts; it gives 12 of 15 even for identical fields and is not a break measure. The same-cell comparison in the last column is.

Breach rule (A-net, `net.rs`): walking donors first, each receiver is lowered to at least 0.2 % below its donor. It carves 8,300 to 19,200 cells per block (mean 45 to 53 m; 61,000 cells, mean 28 m, in the 4 m look run) and leaves 19.7 % of channel cells uphill in the final surface (slumping and diffusion partly undo it); 18 to 22 cells per block are cut from receiver cycles (8 in the truth, 186 at 4 m). The channel bed is not yet a stable one.

Stills: `images/seam_300m.png` and `seam_30m.png` (dynamic; left hard join, right blend) and `seam-net_*.png` (A-net), a CPU heightfield render at 20 m cells, so terraced, not the engine's lit path; they show no visible seam in either variant (the dynamic differences are in channels and slopes, not visible at these ranges).

### 4. Determinism (measurement 4, `raw/determinism.txt`)
| What | 1, 4, 16 threads, two runs each | Result |
|---|---|---|
| coarse run (256, 4 steps, 8 sampling threads in parallel for the per-cell passes) | hash 884bf9e63df11efd in all six runs | pass, bit-exact |
| four dynamic blocks (H 1 km, N 20; threads across blocks) | hash 724048678494befd in all six runs | pass, bit-exact |
`libm` only (pow, exp, sin, cos, floor). Reduction order: the flood's (height bits, index) order for dynamic A; Kahn's queue order from the initial cell index for A-net; accumulation along those orders, sequential inside a block. The per-cell parallel passes of the coarse run (receivers, coefficients) write independent values. **A-net itself was not run under the 1, 4, 16 thread test**; its code path has no parallelism or unordered containers, and the dynamic test covers the shared pieces (sampling, injection with a `BTreeSet`).

### 5, 6. Cost (measurements 5, 6, 9)
Single thread, indicative, 49 km2 at 4 m (`raw/look2.log`) except where marked:
| Variant | Time | s per km2 | ns per cell-step |
|---|---|---|---|
| A dynamic, N = 30 | 18.3 s | 0.37 | 199 |
| A-net, N = 30 | 2.8 s | 0.057 | 30 |
| B, erosion noise (three octaves of creases, slope damped) | 0.51 s | 0.010 | |
| C, droplets (1 M, life 200) and 12 slumping passes | 10.5 s | 0.21 | |
Blocks at 20 m (5 km core, halo H; 4 blocks, mean per block, other sessions running): dynamic A, H = 5 km (750 x 750): N = 10 1.3 s, N = 30 4.7 to 5.2 s (3.2 s when quiet), N = 100 13.9 s; H = 1 km (350 x 350): N = 30 0.9 s. A-net, H = 5 km, N = 30: 0.4 to 0.5 s. A dynamic block costs 130 to 190 ms per core km2 at N = 30 with the 5 km halo, because the halo is nine times the core; A-net's halo is also solved, but at a sixth of the cost per cell.
At 4 m a 5 km core is 1250 x 1250 cells. A-net with the 5 km halo (3750 x 3750 = 14 M cells) would take about 13 s (extrapolated from 30 ns per cell-step); with a 0.5 km halo (1500 x 1500) about 2 s (extrapolated); SPIKE-34 found 5 km blocks at about 6 s work. Dynamic A at 4 m with a 5 km halo would take about 85 s (extrapolated) and would still not meet 1 m.
Blocks around the camera (assumption: fine cells within 1.5 km of the camera): a 300 m view needs 1 to 4 blocks, a 30 m view 1 to 4; until a block is ready the stand-in is the coarse upsample plus base detail, which differs from the block by p50 5 to 60 m, p99 58 to 290 m at N = 10 to 100 (the H = 0 rows above), so the swap has to go through SPIKE-34's level gate.

### 7. Edits (by construction, not measured)
| Variant | What an authored height edit invalidates |
|---|---|
| A dynamic | every block of the drainage basin the edit lies in or drains to (routing is global); a decision for the planner: unbounded |
| A-net | the blocks whose rectangle (core plus halo) the edit touches, 1 to 4; the network stays, so an edit cannot move a river (a limit) |
| B | the footprint of the edit (a point function plus the slope of its input) |
| C | the whole block (one random stream in order) |

### 8. Engine fit (the five rules; verdicts for A-net, dynamic A in brackets)
| Rule | A-net | Notes |
|---|---|---|
| CON-06 pure function of (read-set hash, version, implementation id, tile id) | fits | block = f(coarse state, block id, parameters); the coarse run must be a recorded input of the block (a read of an earlier stage) |
| CON-08 staging, metamorphic tests | fits (dynamic A: border agreement and partition invariance fail) | stage order global-coarse then block (regional, halo) then local; border agreement: the cores of two blocks equal the truth to 0.00 m; partition invariance: any partition into 5 km blocks reads the same field in the cores, a different block origin needs a halo of 5 km to match; coarse/fine agreement needs a stated bound: the block differs from the coarse upsample by p50 5 to 60 m (it carves the fine detail) |
| CON-14 determinism | fits (tested for dynamic A and the coarse run) | libm, fixed order, no thread-dependent reductions |
| CON-22 streaming never blocks | fits with a level gate | block latency 0.5 s at 20 m, about 2 to 13 s at 4 m (extrapolated); the stand-in is the coarse field |
| LOD stability (a tile and its parent agree) | fits only with one fixed raster | a 20 m and a 40 m run of the same block differ (p50 19 m, p99 100 m; initial fields 0.45 / 3.6 m), so tiles at every LOD must be box-filtered from one fixed-resolution block raster |
`height_bounds`: from the block's raster (min and max over the block plus the slumping margin, exact for the raster). The drainage area A (m2) of a block is the wetness input of ADR 0076: A-net's equals the truth's in the core (|ln A| mean 0.000), dynamic A's differs by 0.15 mean at N = 30, H = 5 km.

### 9. Look (measurement 1)
`images/sheet_sunset.png` (sun azimuth 262 degrees, elevation 4 degrees) and `sheet_midday.png` (200 degrees, 55 degrees): hillshades of the central 5 km at 16 m per pixel, panels left to right: **main** (the coarse upsample; flat at this scale, relief 535 m, slope median 6 degrees), **A-net** (relief 775 m, slope median 19 degrees, p99 58), **B**, **C**. Shapes in the central 5 km:
| | Relief | Slope median / p90 / p99 (degrees) |
|---|---|---|
| main | 535 m | 6.1 / 6.9 / 7.4 |
| A-net, N = 30 | 775 m | 19.3 / 41.0 / 58.2 |
| A dynamic, N = 30 | 768 m | 26.2 / 55.8 / 73.0 |
| B | 754 m | 25.9 / 48.3 / 67.0 |
| C | 751 m | 16.4 / 32.3 / 44.6 |
A-net shows connected dendritic rills that feed the straight coarse groove (an artefact of the snapped path) and that follow the 8 grid directions (D8). B has no drainage structure at all (speckle at the three crease scales), C has rounded ridges and no network. None of the three is sharper than SPIKE-32's variants; the engine renders (shadows and material map on, photo sets) were not made in this spike: the lid is closed and the prototype is a CPU tool, so the "contact sheet at 300 m and 30 m with SPIKE-32's shading" of the brief is **not delivered**. The 300 m and 30 m stills here are the seam stills only.

## Open items and what was not done
- **A-net halo sweep** below 5 km (the equivalent of table 2 for A-net), and a shifted block grid (measurement 3's two-grid partition test): not run; the cores equal the truth at 5 km.
- **Breach rule** leaves 19.7 % uphill channel cells; a better bed (grade the groove along the coarse path, not a running minimum) is open.
- **Meander and axis alignment**: the coarse path is a straight segment per coarse cell; D8 rills align with the grid axes (SPIKE-32 found the same).
- **Engine renders** (item 1 of the brief) and A-net under the 1, 4, 16 thread test.
- The coarse run's maximum change per step does not converge (lakes, divides); a lake stage or a fixed-point rule is needed to declare a world version.
- The sites of the dynamic and the A-net measurements differ (coarse 1024 and 2048); the tables are consistent within themselves.
