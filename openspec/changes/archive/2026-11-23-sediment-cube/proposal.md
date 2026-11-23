## Why
Briefing item 10 (inbox #104, A3 section 6.7, G-SED): deposition, one-cell pit spreading and thermal read 8 neighbours and cross divides, so they run per tile after the last round. They refused a cube grid (GEN-144), so they could not run on the cube grid of the compact landmass.

## What Changes
- `deposit`, `spread_pits`, `thermal`, `thermal_all_cells` accept a cube grid (full, masked or compact): the sphere's cell area and distance per cell (`Grid::cell_area`, `Grid::distance_at`) in place of `dx`.
- Volume, not height, is what is kept: thermal and the pit spreading give a neighbour the moved volume over the neighbour's own area; the deposition carries cubic metres.
- Plane grids keep their code path: the output of the default bake does not move.
- GEN-150 added (generator-pipeline).

## Impact
- Code: `generators::bake::{deposit, thermal}`, `tests/bake_compact`. No call site in a pass of the bake.
- Not in this change: K7 and the tile pass still run on plane grids; the sediment steps are not wired to the cube-grid rounds. The breach-lowering departure in K7's deposition (cells the breach lowered count as eroded load; pass 2 has no breach) stays untested and is on the list for G-SED.
