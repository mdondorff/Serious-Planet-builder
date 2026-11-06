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
- IDs: GEN-131 is reserved; a TEST id is needed for the mass-balance and determinism tests.
- The gate's pit p50 of 20 m is a proposal, not the owner's number.
- The Yuan coefficient values (`G` 1, the class scales) are start values from the literature and the class factors, not measured here.
