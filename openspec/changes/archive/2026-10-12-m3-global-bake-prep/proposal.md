## Why

Follow-ups (h) to (k) of ADR 0026's acceptance (opus-planner, status.md M3 item 6), done before the persisting change: the sea level must be exactly an `f32` (a stored `0.1f32` was land in the bake and sea in an `f32` reader), the continentalness doc bound was wrong, production-resolution lattice checks were missing, and GEN-014's memory clause had no tier-A test and no named budget.

## What Changes

- (h) `SEA_LEVEL_F32 = 0.1f32` and `SEA_LEVEL_C` is that value as `f64`; `GLOBAL_VERSION` 2; new known answers (one pointwise value moved, the 16-cell grid hash moved); bit-pattern tests at `f32::from_bits(0x3dcccccd)` (exactly the sea level is sea, the next `f32` up is land) and that `SEA_LEVEL_C` equals the `f32` value as `f64`. (A 20 000-direction f32-versus-f64 agreement loop was written and removed in review: it cannot fail, widening is exact.)
- (i) `GlobalSample::continent` documented as [-1.7, 1.7].
- (j) tier A at a production-like resolution: a 256-cell lattice has exactly 12 (res - 1) + 8 distinct edge directions and 6 (res - 1)^2 + 12 (res - 1) + 8 in all; a block of a level-2 tile is its window of the level-0 lattice bit for bit.
- (k) `bake_block` and `bake_blocks`: a tiled bake (blocks of up to 1024 cells per edge, grids up to 65536 cells per face edge, level at most 12) that hands blocks to a sink in face, row, column order, numbers its tiles from a running index (no list of all tiles) and holds at most one batch of `threads` blocks; `TiledBakeStats::peak_resident_blocks` is counted live between a block's creation and its delivery (numerics review: it was first derived from the batch size). GEN-014's memory clause names the budget (RAM <= 8 GB, CON-21). Thread-count and block-size independence are tested.
- GEN-013 modified (text, scenarios Blocks and Sea level); GEN-014 modified (memory clause).
- Not here: (l) persisting the bake as kind-3 block records with their keys, the `generate` command and its timing, the ADR 0026 point 5 reader ADR.
