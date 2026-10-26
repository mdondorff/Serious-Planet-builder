## Why
Look review 2b (river-r1, 60 m eye, sunset sun 7.5 degrees): the lit wall of the valley is banded in rows with the lighting term alone (albedo, shadow and horizon views are smooth). The same rows show at 60, 200 and 600 m eye heights, so they are not the 4.8 m tile mesh, and dropping the top two crease octaves leaves them. The solved 8 m raster along a wall transect alternates in slope: 0.082, 0.158, 0.082, 0.158 per cell (mean 0.12, the wall's own 0.168 elsewhere), a 16 m sawtooth that swings the normal between 4.7 and 9 degrees.

Cause: `paint_band` takes the wall height `bed + side (x - half_floor)` from `x` = the relaxation's `best`, a sum of edge lengths over the 8-neighbour cell graph; along a wall off the grid axes the path alternates axis and diagonal steps, so `x` gains unevenly per cell.

## What Changes
- The relaxation of `paint_band` carries the owning source and takes, for a neighbour, the Euclidean distance to that source (a vector distance transform) instead of `best + edge length`; ties keep the key order. Nothing else of the band changes.
- `ANET_VERSION` 8 to 9 (a chunk's change differs along every valley wall: every chunk is re-keyed).
- `paint_band` is `pub` so that its test builds a corridor of its own.

## Impact
- Spec: generator-pipeline GEN-083 (new; GEN-060's statement is at its word limit).
- Code: `generators/anet.rs`; test `anet_incision.rs::the_valley_walls_rise_evenly_cell_to_cell_on_a_corridor_off_the_grid_axes`.
- Real site (river-r1 chunk, solved raster dump): the alternation is gone.
- The owner of a cell is the graph's source, not always the true nearest; the vector distance has the small errors of any vector transform near the ridge between two owners. The scallop between two points of the corridor remains the limit; a distance to the corridor's segments is the next step if the look review asks.
- Moved goldens: chunk scenes only if a golden pose reads a valley wall (checked by `cargo xtask test gpu`).
