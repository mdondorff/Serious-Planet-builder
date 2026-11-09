## Why
Gate G-SED (A3 section 12.1) reads "per catchment and per round: eroded = deposited + exported to 1e-9 relative". GEN-151 measured it once on the whole landmass, and from the sweep's own report. It was not measured per catchment, per round, or from the heights.

## What Changes
- `deposit_exports`: the deposition sweep that also reports the load leaving at each outlet cell.
- `sediment::catchment_balance`: the sweep on a surface with the previous round's surface as `original`, and for every catchment of the sweep's route the eroded column against the heights' rise plus the export.
- `examples/landmass_scale sedround`: that balance after each of the 4 rounds on the prototype landmass; `thdiag`: the pits thermal makes, with their neighbourhoods.
- GEN-152 added (generator-pipeline).

## Impact
- Code: `generators::bake::{deposit, sediment}`, `examples/landmass_scale`, `tests/bake_compact`. No call site in a pass of the bake.
- G-SED stays partly open: closed for the deposition sweep run on a round's result, open for sediment inside a round, the round's own catchments, thermal and the pit spreading.
- Not in this change: the round solver itself tracks no sediment (its carve removes material without accounting for where it goes); the balance here is the deposition sweep run on each round's result. A sediment step inside the round is not built.
