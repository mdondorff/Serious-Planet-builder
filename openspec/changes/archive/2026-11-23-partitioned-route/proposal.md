## Why
A3 (docs/design/terrain-target.md) sections 6.5 and 6.6 and inbox #98 step 2: the region's routing is computed as tiles with memoised tile floods, and equals the whole-region routing exactly (gate G-PART). This change is the partitioned route behind a switch, stacked on rounds-prototype (GEN-142).

## What Changes
- `bake::partition`: `partitioned_route` (tile floods from the values the neighbouring tiles offer, a memo of tile floods by their inputs, per-tile receivers, the area by a topological sweep over the tiles) and `PartCache` (the memo and the previous result, for the change counts).
- GEN-143 added (generator-pipeline): the partitioned route equals `route_rules` under the round rules (filled levels, receivers, `A_d`, order), for any tile size and on edited surfaces, and an edit floods only the tiles whose inputs changed.
- `route::receiver_of` and `route::topological_order` visible to the crate (the partitioned route runs the same functions).
- `examples/partition_region`: G-PART on the prototype region (1,555 x 1,045 cells, 256-cell tiles) and two E2-type edits, with the tile floods run and the tiles changed printed.

## Impact
- Code: `generators::bake::partition` (new), `bake::route` (two functions made `pub(super)`, `receivers` split without a change of behaviour), `examples/partition_region`. No call site in pass 1 or pass 2; nothing in today's output moves.
- Not in this change: the landmass across cube edges (GEN-144, next); a non-resident partitioned route (the arrays stay resident here); a memo of the area sweep and the receivers (recomputed whole, reported).
