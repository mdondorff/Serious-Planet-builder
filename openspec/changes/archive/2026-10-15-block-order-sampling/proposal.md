## Why

M4 gated dive (20,000 km to 2,000 km). The tile generation path evaluates the samples of a tile in raster order. Over the stored bake a coarse tile spans tens of blocks, and consecutive samples of a row jump from block to block, so the shared byte-budgeted cache (31 blocks) thrashes: a cold root decodes about 7.5 times the blocks it touches (18,147 decodes for 1,536 distinct blocks over six roots; 103 s with six roots in parallel, where one root with an unbounded budget takes about 7 s). Slow coarse tiles mean more than three levels of lag in the dive. Decision of opus-planner (2026-10-15, delegation): evaluate the samples of a tile grouped by the bake block their lattice reads fall in, so each block is decoded about once per tile.

## What Changes

- generators: `HeightField::heights_until(dirs, stop)` / `heights(dirs)` batch methods (default: a loop over `height`); `LatticeSource::block_cells()` (default 0: no block structure); the lattice-reading fields (`ErosionField`, `TerrainField`) override the batch with a two-pass gather: collect the lattice cells the whole batch reads, read them from the source grouped by block (each block decoded once), evaluate from that table in the caller's order. (Sorting the samples by the block of their primary cell alone was tried and measured first; see the design.) Every sample stays a pure function of its direction, so the output is bit-identical to the raster order.
- `tile_mesh_field` and `compact_tile_field` collect every direction they need (lattice samples, normal stencil points, the origin) and make one batch call. `surface_normal_field` is split into its stencil directions and the normal from four heights, with the same arithmetic.
- `Bound` and the app's `CancelGuarded` delegate the batch; the batch polls the stop flag per block group, every 64 cells and every 64 samples, so teardown still cancels promptly.
- cache: `StoreSource::block_cells()`.
- app example `store_blocks`: `--order raster|block` switch, `--part tiles` (per-level tile times, decodes against distinct blocks) and a lattice-access recording of distinct cells read.

Not in this change: keys, pack or bake format, `MESH_VERSION`, goldens, pinned counts, any coarse summary or lattice pyramid.
