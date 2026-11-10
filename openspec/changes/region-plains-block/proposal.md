# region-plains-block (briefing item 13, step 2), example only

Architect 2026-11-10 (inbox #103): the stage probe is ON during the item-13 runs and the plains block is defined before it is measured.

## What
- `crates/generators/examples/region_scale.rs` installs a `SheetProbe` (sheets and numbers in `target/review/stages/region-s<seed>-f<face>-i<i0>-j<j0>/`, nothing committed; `PLANET_STAGES=0` switches it off) and emits the H1 stage (area, z, filled).
- The weighted route and the canonical flood are timed apart; the flood's share of the route-and-flood step and the total run time are printed.
- Plains block, over the land cells of the region proper on the carved surface: false flats (a receiver and a filled-surface drop to it of exactly 0 or below one f32 step at that height), sinks (cells the fill raised, with the mean raise) and pits (no receiver, not an outlet), each as count and share.
- No requirement text, no library code, no test: an example is a measurement tool.

## Not in this change
- The runs and their results (`spikes/prototype-gates/results/results.md`).
