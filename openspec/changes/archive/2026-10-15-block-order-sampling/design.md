## Where the order lives

`HeightField::heights_until(&self, dirs, stop)` returns `height(d)` for each `d` in the caller's order. The default loops. `ErosionField` and `TerrainField` read a `LatticeSource`; when it reports `block_cells() > 0` they call `global::eval_gathered`, a two-pass batch: (1) a footprint pass collects every lattice cell the directions will read (the field's own read path over a recording source that answers a placeholder; the footprint depends on the directions only); (2) the distinct cells are read from the real source grouped by block (face, then Z order of the block), so a block is touched by one run of reads and decoded once however small the cache; (3) the field's `height` is evaluated from the in-memory table of those cells, in the caller's order. A cell is the source's own sample (or its error), so every result is bit-identical to `height(dir)`; no value is combined across samples, so no reduction order exists. Sorting the samples by the block of their primary cell was tried first and measured: a sample near a block border also reads its neighbour, and with a shared cache of 31 blocks across six parallel roots it still decoded 3.4 times the distinct blocks (learnings, 2026-10-15).

## Cancellation

`stop` is polled before each block group, every 64 cells of the gather and every 64 samples of the evaluation; once true the remaining slots are 0.0 and no further read happens. `CancelGuarded` passes `stop || cancel.is_cancelled()` to the inner field. A cancelled tile is dropped by `load_staged`, as before.

## Tile builders

Both tile builders need `(cells + 1)^2` heights, four stencil heights per vertex for the normals, and the origin height. They build one direction list, call `heights` once and consume the results with the original arithmetic (`surface_normal_field` = `normal_stencil` + `normal_from_stencil`). Memory: about 5,450 directions per tile at cells 32.
