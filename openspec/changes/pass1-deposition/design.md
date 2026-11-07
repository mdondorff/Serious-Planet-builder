## Context
- ADR 0105 points 1 and 3; ADR 0101 (kept pits, fill-depth channel); ADR 0088 point 4 as amended by ADR 0099; ADR 0108 (Earth is the default preset).
- Today: `carve_rock` (carve.rs) solves implicit stream power per step in the stack order (receiver before donor), `keep_pits` leaves a pit cell with its uplift only, and no mass moves downstream. Pass 2 has its own explicit transport-limited sweep (`deposit.rs`, GEN-101); pass 1 has none.
- Pass 1: 12,288 cells a face edge, 20 steps, re-route every 5 steps (flood, MFD area, order), one face at a time, serial sweeps.

## Decisions
1. Method: Yuan et al. 2019 (stream power with deposition, FastScape form), not an explicit volume router.
   - Per cell: `dh/dt = U - K sqrt(Q) S + G Qs / Q`, `Qs` the sediment flux through the cell, `G` a dimensionless deposition coefficient.
   - `Qs_i = sum over donors Qs_j + dx^2 (U - dh_i/dt)`: the cell's own erosion volume rate added to what the donors bring.
   - The step stays implicit and linear in `h_i`: a sweep from the leaves to the outlets accumulates, per cell, the affine relation `h_j = a_j + b_j h_receiver`; a sweep from the outlets back down the stack solves the heights. Two passes over the stack order the carve already has.
   - Reason: it keeps the stability of the implicit carve at the step sizes pass 1 uses (f 2-6 on trunks, GEN-127) and needs no extra routing; an explicit router would need a CFL-limited sub-step on steep trunks.
2. Pits: a cell's deposition is capped at its spill level, the level the flood gives it (`filled - z`).
   - The excess is not lost: a third sweep (leaves to outlets) re-accumulates `Qs` from the final heights, so the mass balance is exact and the excess moves on.
   - A pit receives only what is eroded upstream of it, so a pit with a small catchment stays deep; one with a large catchment fills to its spill and stops being a pit. That is the wanted behaviour (ADR 0105 context).
3. Outlets and the sea: sediment reaching an outlet leaves (as in `deposit.rs`); no deposition on sea cells; the carve's sea-level floor stays. Deposition on land cells at the coast builds the coastal plain; shelf and delta building belong to `coastal-process` (GEN-135).
4. Class and rock: `G` is scaled per cell by the class carve factor's deposition share (bedrock channels in mountains deposit less than alluvial plains); the table below holds the start values, tuned in the measurement.
5. Determinism (CON-14): fixed iteration count, the existing total order (stack order, then cell index), serial sweeps, `libm` for `sqrt`, no thread-dependent sums; the same bits at 1 and 8 threads as today's carve.
6. Pass 2: unchanged. It reads pass 1's change (C1) and the fill depth; pass 1's face flood runs after deposition, so kind 16/17 depths shrink by construction. `PASS2_KERNEL_VERSION` does not change unless the measurement shows a reason.
7. Keys: `BAKE_KERNEL_VERSION` rises; the deposition parameters (G, class scale, the pit cap) are in the key through the pass-1 parameter digest, so a layer baked without deposition is refused (owner, 2026-11-05: older stores need not stay compatible). `arid-sinks` (GEN-132) reuses the sink rule of point 2 for closed arid basins.

## Cost estimate (to be measured; an estimate, not a measurement)
- Sweeps: the carve does one stack sweep a step; this adds three (two affine passes, one flux re-accumulation), each of the same memory-bound shape. The flood, MFD sort and order build every 5th step stay the larger share.
- Estimate: pass 1 +15 to 30 % of today's time, so +0.2 to +0.45 h on the 1.5 h pass-1 target at B = 10 (thread-hours scale with the same ratio; one face is serial).
- Memory: `Qs` (f32) and two affine coefficient arrays (f32) over the face with halo (1.56e8 cells): about 1.9 GB, on top of the +1.55 GB carve item. CON-21's 8 GB budget is at risk: the integration run (inbox #89, item 1) records the peak. Fallback: reuse the `a` array for `Qs` (the re-accumulation needs `Qs` only after the solve), about 1.2 GB.
- Bake date effect: none beyond the pass-1 time, if the measured ratio holds.

## Measurement plan
| What | Where | Baseline | Proposed gate |
|---|---|---|---|
| Pit depth p50, p95, share of cells in a pit, per class | tiles 2/12/10 (mountains), 0/8/24 (hills), one basin site (intermontane) | keep-pits today (p50 63 m in mountains); GLO-30 at the same grid scale (priority flood of the Everest, Alps and one basin tile, resampled to 814 m): p50 3-7 m | p50 at most 20 m; p95 reported |
| 3 km relief p50 per class | same tiles, the spike's measure on the filled surface | PR 1 baseline 519 / 675 m (carve-k-u table) | at least 0.9 of the keep-pits value (deposition must not eat the relief the pits kept) |
| Mass balance | synthetic face and 2/12/10 | eroded = deposited + exported | relative error at most 1e-9 |
| Valley floor flatness | one alluvial valley of the hills tile | none | floor slope below the carve's own slope at the same area; stills |
| Determinism | 1 and 8 threads | - | bit-equal |
| Cost and memory | face 2 at 12,288 single thread, ratio to today | today's time | within the estimate above |

Stills: 2/12/10 at 2 km and 10 km, low sun, deposition on and off, same pose.

## Earth-specific values (ADR 0108 point 4)
| Name | Default | Unit | Kind | Parameter that holds it |
|---|---|---|---|---|
| `DEPOSIT_G` | 1.0 (start; tuned) | 1 | calibration (Yuan et al. 2019 used 1 for alluvial reaches) | `Pass1Params.deposit_g` |
| `DEPOSIT_CLASS_SCALE` | mountains 0.25, hills 0.5, plains 1 (start; tuned) | 1 | calibration | the class table of the class source (GEN-098 style) |
| Spill cap | the flood level, `epsilon` 1 mm | m | law (a pit stops filling at its spill) | `CarveParams.epsilon` |
| Sea level | 0 | m | planet state | `CarveParams.sea_level` |

The constants are module constants until the ADR 0108 audit lifts them into the world definition; calibration factors are fixed constants of the preset, never recomputed per world.

## Risks
- Deposition fills every kept pit and the relief gain of keep-pits is lost: the relief gate and the class scale are the control; the planner and owner decide if the gate fails.
- The clamp at the spill level makes the solve piecewise: the third sweep keeps mass exact, but the heights after a clamp are not the exact implicit solution; the difference is one step's worth and is reported in the test.
- Memory, above.

## Open points for the planner
- IDs: GEN-131 (carve) and TEST-095 (mass-balance and determinism tests) are reserved for this change (planner, 2026-11-06).
- The gate's pit p50 of 20 m is a proposal, not the owner's number.
- The Yuan coefficient values (`G` 1, the class scales) are start values from the literature and the class factors, not measured here.

## Findings (2026-11-07, seed 14, tile 2/12/10, face 2 at 12,288; measured, not a gate)
| Setting | Pit share of land | Deepest pit | Pit depth p50 / p95 (pit cells) | Change range | Pass 1 face time |
|---|---|---|---|---|---|
| G = 0 | 11.9 % | 710 m | 139 / 467 m | -1522 to 619 m | 1235 s |
| G = 1, receiver ceiling | 12.4 % (faces 0 and 2 run) | 826 m | not printed | -1496 to 791 m | about +4 % |
| G = 1, ceiling relaxed to the flood level (probe `PLANET_DEPOSIT_CEILING=flood`) | 12.2 % | 816 m | 139 / 482 m | -1492 to 798 m | 1799 s (probe floods each step) |
| G = 4, same probe | 12.9 % | 830 m | 145 / 527 m | -1448 to 1193 m | 1784 s |
- Relaxing the ceiling does not fill the pits, and G = 4 raises the change range instead (up to +1193 m): the eroded volume of 20 transient steps is small against pits hundreds of metres deep.
- Reading (a hypothesis): most of the pit volume is in the base surface's own basins kept by ADR 0101, not made by the carve, so a deposition term fed only by the carve's erosion cannot reach the 20 m p50 gate.
- Relief against G = 0 is not measured (no relief tool on this branch).
- Process peak 10.0 GB at G = 0 (above CON-21's 8 GB; an M9.1 item of the planner), 10.9 GB with the probe's flood.

## Basin infill before the carve (ADR 0109, GEN-137; owner D7 a)
Supersedes the pit goal of the deposition findings above: deposition stays for fans, deltas and valley floors, its check is mass balance and flatness.
1. Method: the flood of the base surface (GEN-100's priority flood, `epsilon` over flats) gives every cell its spill level. A basin is a connected set (8 neighbours) of cells more than 0.5 m (`PIT_MIN_M`, the minimum of GEN-120's fill-depth channel) under it. A basin smaller than `min_area_km2` or shallower than `min_depth_m` at its deepest cell is raised to the flood; any other is raised to the flood minus `residual_m` where it is deeper.
2. Properties (tests): never raises a cell above its spill level, never lowers one, keeps outlets, same bits on every run, idempotent (the residual is at least the minimum depth, so a basin left is not filled by a second infill), no basin left under the thresholds.
3. Where: `pass1_face_rock` runs it on the sampled base surface before the carve; the stored change holds the infill, so base plus change is the infilled and carved surface. The fill-depth channel then measures what is left after the carve (the lake basins). Arid basins stay sinks (ADR 0105 point 2, `arid-sinks`).
4. Switch: `CarveParams.infill` (`None` = off, today's surface); the thresholds are in the layer keys. `bake layer --infill AREA,DEPTH,RESIDUAL`.
5. Cost: one more flood and one labelling walk of the face (about one routing's flood, once per face, not per step), a `u32` label plane (0.6 GB at 1.56e8 cells) and the flood surface (1.25 GB) beside the face arrays: a transient peak before the carve's arrays exist. To be measured; the 10 GB process peak is the planner's M9.1 item.

| Name | Start values | Unit | Kind | Parameter |
|---|---|---|---|---|
| `min_area_km2` | 25 (set 2: 100) | km^2 | calibration (Earth's lake size distribution, HydroLAKES, Messager et al. 2016: lakes above 10 km^2 are a small share of lakes but most of the area) | `Infill.min_area_km2` |
| `min_depth_m` | 50 (set 2: 100) | m | calibration (typical lake depth of mid-size lakes) | `Infill.min_depth_m` |
| `residual_m` | 100 (set 2: 200) | m | calibration (mean depth of the large lakes, Great Lakes 20-150 m; the deep rift lakes are the tail) | `Infill.residual_m` |
Target of the check: lake share of land 2-3 % (Earth), basin depth per class against GLO-30.

## Findings: where the pit volume is (inbox #96; seed 14, face 2 at 12,288 cells, G = 0, pits = cells more than 0.5 m under the flood; measured at pass 1's grid by `pit_compare`)
| Surface | Scope | Pit share of land | Depth p50 / p95 / deepest | Volume |
|---|---|---|---|---|
| raw base | tile 2/12/10 footprint | 31.5 % | 381 / 1299 / 1818 m | 1.49e13 m3 |
| raw base | whole face | 34.7 % | 11 / 480 / 1947 m | 8.61e14 m3 |
| base + pass-1 change | tile 2/12/10 footprint | 0.2 % | 0.9 / 2.0 / 2.9 m | 1.9e8 m3 |
| base + pass-1 change | whole face | 9.0 % | 5 / 15 / 400 m | 2.09e13 m3 |
- The carve does not make the pit volume: the raw base holds it, and pass 1's breach and carve remove most of it at pass 1's grid (the face's pit volume falls to 2 %, the tile footprint's to nearly nothing).
- The pits the bake prints after pass 2 (11.9 % of land, p50 139 m, deepest 710 m) therefore stand at pass 2's 102 m grid: the surface there is the fine base plus the C1-read pass-1 change, and dams of the fine base that the 814 m breach does not reach remain basins. A fix at pass 1's grid (the infill on the 814 m base) does not address them directly.
- Basin infill probe (the same tile, `--infill` AREA,DEPTH,RESIDUAL, pass-2 fill pit stats): 25 km2 / 50 m / 100 m: 8.3 % of land, p50 14.5 m, p95 99 m, deepest 386 m, change range up to +1759 m; 100 km2 / 100 m / 200 m: 9.8 %, p50 19.5 m, p95 107 m, deepest 364 m, +1659 m. Against G = 0 (11.9 %, p50 139 m, deepest 710 m) the share barely moves, the depth falls strongly; lake share is far above Earth's 2-3 %. The infill raises the base by up to 1.7 km in the deepest basins.
