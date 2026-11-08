## Context
- GEN-141 (step 1) made the clipped MFD area and the canonical flood available behind `RoundRules`. A3 section 6.2 says a round is one region-wide routing and then per-catchment breach and sweeps on the shared routing.

## Decisions
1. A catchment's inputs are gathered into a `CatchmentInputs` value (members in the region's topological order, local receivers) and the round runs on it alone (`catchment_round`). The solver calls the same function the tests call, so G-LOCAL tests production code.
2. Threads: a shared queue, the largest catchment first; workers return the new heights and the main thread scatters them after the join. No shared mutable state, no `unsafe`. Lone outlets (sea cells) are skipped.
3. The taper of the uplift is taken from the heights at the start of the solve, as `carve` does.
4. The window of the prototype is the region plus a margin; its border cells are outlets (an artificial cut). The landmass is not routed whole here (cube-edge grid: later). The region's change is the same for margins 200 and 700 (below).
5. Out of scope here: rock layers, class factors, runoff weights, the area cap (asserted off); G-PART (partitioned route); G-SCALE at landmass size.

## Measurements (`examples/rounds_region`, release, 4 rounds of 5 steps, true metric, round rules, seed 14)
| Window | Cells | Land | Route per round | Cut | Carve (1 thread) | Carve (8 threads) | Whole-region carve | G-ROUND | G-DET |
|---|---|---|---|---|---|---|---|---|---|
| margin 200 (555 x 545) | 302,475 | 61.7 % | 0.19 s | 0.01 s | 0.02 s | 0.05 s (4 rounds) | 1.22 s | 0 cells differ | 0 |
| margin 700 (1,555 x 1,045) | 1,624,975 | 35.1 % | 0.65 s | 0.06 s | 0.11 s | 0.54 s (4 rounds) | 6.66 s | 0 | 0 |
- The routing is about 85 % of a round, as in F7. The carve with 8 threads is not faster than with 1 on these windows: about a million catchments are single sea cells or tiny, and the per-catchment gather allocates. Batching small catchments is an option when the carve matters.
- Region land change after 4 rounds, region box only: n 16,730 land cells, p5 -565.4 m, p50 -132.4 m, p95 -1.7 m, max +16.0 m. Identical for both margins: the region's result does not depend on the window's border.
- Largest catchment: 55,385 cells (margin 200), 75,346 cells (margin 700).

## Earth-specific values (ADR 0108 point 4)
| Value | Earth | Rule |
|---|---|---|
| Grid | 3,072 cells a face edge, 3.26 km, cell 10.6 km^2 | the cells come from the planet's radius and the face resolution (`FaceSpec::dx`) |
| Round | 5 steps, 4 rounds | `CarveParams::reroute_every`; the rounds count is the profile's (A3 section 8) |
| Radius | 6,371,000 m | the example's; the solver reads the grid only |
