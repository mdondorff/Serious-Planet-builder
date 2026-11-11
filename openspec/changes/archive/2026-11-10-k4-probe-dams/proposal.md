## Why
Architect, 2026-11-10: the due k=4 re-measure with Lakes in the build is a production phase plan run with the stage probe on and the dam count. Item 10's question is whether the pits that remain after the pass are made by the pass. The example's plan mode installed no probe and counted no dams.

## What Changes
- `examples/landmass_scale plan`: the stage probe is installed as in `region_scale` (sheets of at most `PLANET_STAGE_BINS` bins, default 4,194,304, in `target/review/stages/plan-k<k>/`; `PLANET_STAGES=0` switches it off; its memory is inside the printed peaks).
- The dam count (`PLANET_DAMS=0` switches it off). The land pits after the whole pass (canonical flood, 0.5 m, as G-LAND) are 8-connected components. A component is a DAM when the pass raised its spill cell (the lowest cell of its rim: `z_after - z_before > 0`) and the pit is new or at least 0.5 m deeper than before the pass; `z_before` is the surface after the last round, before thermal. Otherwise it is inherited. The cause of a dam is the raise at the spill cell by thermal, by the deposition, by the pit spreading or by several (counted by combination). Printed: the pit cells after the pass split into dams and inherited (components, cells, share of the land), the dams by cause, the dams' depth p50, p95 and maximum, the dams deeper than 20 m, the top 10 by depth (a pit cell, the depth, the raise at the spill cell and its cause). The one-cell wall (the outside cell beyond the spill along the line from the pit more than 5 m lower and both cells across the line higher) is an extra column, not the definition.
- How the surfaces are held: a stage probe (`DamProbe`) writes the surface `z` of three stages to `target/plan_dam_k<k>_<pid>_{before,thermal,deposition}.f64` as they pass (the last round, thermal, the deposition) and holds nothing, so the build's own peaks are the build's. The analysis reads them back after the `THE COMPLETE BUILD (plan)` line (four surfaces and two fills, about 6 GB at k=4) and prints its own memory line; the files are deleted after the count (disk use 8 B a cell each: about 3.6 GB at k=4, `target/` only).
- `landmass_scale 1 8 damtest`: the report on a fixture (a plateau, a one-cell canyon raised by thermal in the "after" surface, a closed basin in both): 2 pits, 1 dam (cause thermal, also a wall), 1 inherited.

## Impact
- Code: the example only; no library code, no bake bit moves.
- k=1 plan run with the change (target/review/plan_k1_dams_d15.log): see the PR body.
- Not in this change: the k=4 runs (they start from this pushed commit).
