Opened: 2026-10-23
Question: Does a coupled landscape-evolution simulation over time (stream-power erosion and drainage routing in one loop, hillslope diffusion, uplift) fit this engine's stage model as a coarse global run plus a bounded number of fine steps per fixed block, and does it reach better landforms at an acceptable offline and runtime cost than droplet erosion or per-point erosion noise?
Why it matters: M8 "Natural landscape" starts implementation on 2026-10-25 (owner); the stage model (CON-08 reading, fixed block grid, sub-stage order against a coupled simulation) and the landform method are decided from this spike (owner, 2026-10-23).
Time box: about 1 day of agent work, indicative timings on the RTX 3080 Ti Laptop (other sessions running), no perf window; report by 2026-10-24 evening.
Success criteria: the table and stills of the brief below for variants A, B and C at SPIKE-32's site; the light-cone, partition and determinism measurements for A; the offline and runtime cost of A at the stated grids; a fit verdict per engine rule (CON-06, CON-08, CON-14, CON-22, edit invalidation, LOD stability).
Fallback if it fails: a written list of what blocks variant A in this engine, and B or C as the M8 landform method.
Code location: branch `spike/landscape-evolution` (never merged); a prototype patch against main in this folder.

# SPIKE-33 brief: landscape evolution over time (landscape-evolution)

## Variants
| Id | Method | Notes |
|---|---|---|
| A | Coupled stream-power landscape evolution: per step route the flow (receivers, drainage area A) over the current heights, then the implicit stream-power update `dh/dt = U - K A^m S^n` (Braun and Willett 2013, O(n) per step, stable at large dt) plus hillslope diffusion | m 0.4 to 0.5, n 1; K and U by rock and region; a glacial term is out of scope |
| B | Per-point erosion-like noise (derivative-damped or gully noise over the base field), no halo | the local baseline |
| C | Droplet erosion as in SPIKE-32's `RegionStage` | the current baseline |

## Two-scale scheme for A
1. Coarse global run: the whole planet at 2048 cells per face edge (about 5 km), from the global field, enough steps to reach a steady relief (report the step count and the convergence curve); outputs heights, drainage area and sediment.
2. Fine run per block: a fixed block grid (blocks of whole coarse cells, as in SPIKE-31), cells of 4 to 10 m, starting from the coarse state upsampled plus the base field's detail, N fine steps, the inflow at the block border from the coarse drainage area (SPIKE-31's injection), a halo of H.
3. Truth: the same fine run over the whole 20 to 40 km region at once.

## Measurements
| # | What | Output |
|---|---|---|
| 1 | Look: stills at SPIKE-32's site and poses (sunset and midday, 300 m and 30 m) for main, A, B, C, with SPIKE-32's shading (shadows and material map on) | a contact sheet under 1 MB |
| 2 | Light cone of A: block-against-truth error in heights (m) and ln A for N in {10, 30, 100} and H in {0.5, 1, 2, 5} km; the smallest H per N under 1 m height error at p99 | table; the H(N) rule |
| 3 | Partition of A: two block grids (shifted by half a block) with the chosen H; seams in heights and river size at block borders | table, a difference image |
| 4 | Determinism of A: bit-exact heights for 1, 4 and 16 threads and two runs; `libm` only; the reduction order stated | pass or fail |
| 5 | Offline cost of A: the coarse global run (time, threads, memory, steps); extrapolate a 4096 run | table |
| 6 | Runtime cost of A: ms per block and per km2 for each N, threads; the blocks a 300 m and a 30 m view need around the camera; the latency until a block is ready (a stand-in from the coarse field until then, CON-22) | table |
| 7 | Edits: the area an authored height edit invalidates under A (the light cone of N steps), against C and B | one line each |
| 8 | Engine fit: A as a `HeightField` over the coarse store (`height_bounds` from the block's run), LOD stability (a fine tile and its parent agree), the drainage area as ADR 0076's wetness input | a verdict per rule |
| 9 | Cost of B and C at the same site, per km2 | table |

## Rules for this spike
- Build on SPIKE-32's patch (`spikes/realism/results/realism-prototype.patch`) and SPIKE-31's patch layout; spike code stays on its branch.
- The owner's 16384 store is read-only; copy what a run modifies into your own `target/`.
- No `cargo xtask perf`; GPU and CPU timings are indicative (median of 60 frames, other sessions noted).
- Report: numbers in tables, a recommendation with the stage model it implies (blocks, halo, step cap), and a Visuals section.
