## Why
Criterion 6 (owner): the eroded result of an edit on screen within 10 s for every touched chunk the view wants. The horizon bake of a chunk is a job after the chunk solve (STRM-048). One sample on the real 8 m raster (2026-10-26, busy machine, `target/review/ripple/raster.bin`): raster 1824 x 1824 built in 4.0 s, march 11.8 s, total 15.9 s; the chunk solve is 3.3 s.

Offline sizing (4 threads, same machine, 608 x 608 block, 16 azimuths):
| Variant | March |
|---|---|
| Today | 6.8 s |
| Max-mip skip (block max of the height, exact test) | 6.5 to 8.7 s: no gain; the cost is the arithmetic per step, not the loads |
| Step table (d and d^2/2R made once) and an integer floor | 2.6 s, byte-identical |
The raster evaluates the field at 3.3M points, 89 % of them in the eight neighbours' cells that up to nine chunks each evaluate again.

## What Changes
1. **The march reads a step table** (GEN-085): the distances and curvature drops are made once per bake, the floor is an integer floor; the output is byte-identical to today's (test against a copy of the old march).
2. **Height tiles** (STRM-059): the baker keeps the heights of chunk-sized blocks (614 x 614 f32 = 1.5 MB) in a bounded cache keyed by everything the heights depend on; a bake assembles its raster from the nine blocks around its chunk, so a chunk evaluates the field for the blocks nobody has, about one block in steady state instead of nine.
3. Not in this change: lockstep or SIMD over azimuths (measured afterwards as a number only), a coarser horizon texel (owner), a GPU bake (owner option).

## Impact
- Spec: GEN-085 (new), STRM-059 (new); GEN-044 and STRM-049 are unchanged (same bytes).
- Code: `generators/src/horizon.rs`, `app/src/horizonworld.rs`; tests `generators` unit, `app/tests/horizon_world.rs`.
- Memory: at most 64 height tiles, 96.5 MB at the 614-cell chunk (design.md).
