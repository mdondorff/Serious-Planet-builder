# region-scale-window (briefing item 13, step 1), example only

Architect 2026-11-10 (inbox #103): the structural gates run on a second region and seed (R3a, then R3b of SPIKE-106's survey).

## What
- `crates/generators/examples/region_scale.rs` takes `PLANET_REGION=seed,face,i0,j0,size` (cells at 12,288 a face edge; the window is the start and `size` cells, the margin as before). Without it the region is unchanged: seed 14, face 1, start (8056, 760), 620 cells.
- No requirement text, no library code, no test: an example is a measurement tool.

## Default unchanged
`region_scale gates 0 2 128` before (origin/main) and after: identical output apart from timings (window, land share, G-DET, G-ROUND, G-PART, H1 stats, G-CON values, channel density).

## Not in this change
- The stage probe and the plains block (false flats, flood share): the next change.
