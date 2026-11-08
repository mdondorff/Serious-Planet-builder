## Context
- GEN-141 (step 1) made the clipped MFD area and the canonical flood available behind `RoundRules`. A3 section 6.2 says a round is one region-wide routing and then per-catchment breach and sweeps on the shared routing.

## Decisions
1. A catchment's inputs are gathered into a `CatchmentInputs` value (members in the region's topological order, local receivers) and the round runs on it alone (`catchment_round`). The solver calls the same function the tests call, so G-LOCAL tests production code.
2. Threads: a shared queue, the largest catchment first; workers return the new heights and the main thread scatters them after the join. No shared mutable state, no `unsafe`. Lone outlets (sea cells) are skipped.
3. The taper of the uplift is taken from the heights at the start of the solve, as `carve` does.
4. The window of the prototype is the region plus a margin; its border cells are outlets (an artificial cut). The landmass is not routed whole here (cube-edge grid: later). The region's change is the same for margins 200 and 700 (below).
5. Out of scope here: rock layers, class factors, runoff weights, the area cap (asserted off); G-PART (partitioned route); G-SCALE at landmass size.

## Measurements (`examples/rounds_region`, release, 4 rounds of 5 steps, true metric, round rules, seed 14, one machine)
| Window | Cells | Land | Solver, 1 thread | Solver, 8 threads | Whole-region carve (20 steps) | G-ROUND | G-DET |
|---|---|---|---|---|---|---|---|
| margin 200 (555 x 545) | 302,475 | 61.7 % | 0.90 s | 0.93 s | 1.22 s | 0 cells differ | 0 |
| margin 700 (1,555 x 1,045) | 1,624,975 | 35.1 % | 3.45 s | 4.72 s | 6.66 s | 0 | 0 |
- Routing per round: 0.19 s and 0.65 s (about 85 % of a round); cut 0.01 s and 0.06 s.
- 8 threads are not faster than 1 on these windows. Catchments: 117,016 and 1,057,253, most of them one cell or a few; each gather allocates.
- Region land change after 4 rounds (region box, n 16,730 land cells): p5 -565.4 m, p50 -132.4 m, p95 -1.7 m, max +16.0 m. Equal at margins 200 and 700 on seed 14.
- Largest catchment: 55,385 cells (margin 200), 75,346 cells (margin 700).

## Earth-specific values (ADR 0108 point 4)
The example is a measurement, not a profile; the solver reads the grid and `CarveParams` only.
| Value | Earth (example) | Rule |
|---|---|---|
| Grid | 3,072 cells a face edge, 3.26 km, cell 10.6 km^2 | from the radius and the face resolution (`FaceSpec::dx`) |
| Region | face 1, i 2014-2168, j 190-344 | the prototype region's definition (results.md) |
| Seed, sea level | 14, 0 m | the world's seed and sea level |
| Round | 5 steps, 4 rounds | `CarveParams::reroute_every`; the round count is the profile's (A3 section 8) |
| Carve parameters | `CarveParams::default()` (U/K 20, taper 60 m, breach slope 1e-3) | the pass-1 start values; the profile owns them |
| Radius | 6,371,000 m | the planet's |
