## Why
A3 (docs/design/terrain-target.md) section 6.2 and inbox #98 step 2: the prototype's erosion solver is a round. The kernels of step 1 (GEN-141) make a catchment's round a function of its key inputs; this change is the solver that runs rounds on them, on the prototype region, behind a switch.

## What Changes
- `bake::round`: `CatchmentInputs` (a catchment's key inputs: members in region order, local receivers, `A_d`, distances, taper, surface), `catchment_round` (breach and sweeps on those inputs alone), `cut`, `gather`, and `solve_rounds` (per round one region-wide routing under `RoundRules::ROUND`, then every catchment on a shared work queue, results assembled after the join).
- GEN-142 added (generator-pipeline): the solver equals the whole-region carve with the same rules (G-ROUND) and is independent of the thread count (G-DET at kernel scale); a catchment's round reads its key inputs only (G-LOCAL).
- `examples/rounds_region`: the prototype region (face 1, i 2014-2168, j 190-344) plus a margin, 4 rounds of 5 steps, true metric, with the gates and the cost split printed.

## Impact
- Code: `generators::bake::round` (new), `bake::carve::distance` visible to the crate, `examples/rounds_region`. No call site in pass 1 or pass 2; nothing in today's output moves.
- Not in this change: the partitioned route with memoised tiles and G-PART (next change); the landmass routed whole across cube edges (the window's border is an outlet here).
