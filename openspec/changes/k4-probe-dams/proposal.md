## Why
Inbox #124 (architect, D12 and D13): the due k=4 re-measure with Lakes in the build is a production phase plan run with the stage probe on and the dam count. The example's plan mode installed no probe and counted no dams.

## What Changes
- `examples/landmass_scale plan`: the stage probe is installed as in `region_scale` (sheets of at most `PLANET_STAGE_BINS` bins, default 4,194,304, in `target/review/stages/plan-k<k>/`; `PLANET_STAGES=0` switches it off; its memory is inside the printed peaks), and after the final peak line the dam count of the final surface is printed (`PLANET_DAMS=0` switches it off).
- The dam definition (the architect confirms or changes it): among land pits (canonical flood, 0.5 m) deeper than 20 m, a dam is a pit whose spill cell has the outside cell beyond it along the line from the pit more than 5 m lower and both cells on either side across that line higher: a one-cell-thick wall across a cut (the thermal dams of infilled one-cell canyons, sed-catchment). Components are 8-connected; the cross-axis test is exact inside a face.
- `landmass_scale 1 8 damtest`: the count on a fixture (a plateau, a one-cell canyon with a raised cell, a closed basin): 2 pits deeper than 20 m, 1 dam.

## Impact
- Code: the example only; no library code, no bake bit moves. A k=1 plan run with the change (probe on, 26 MB of sheets): H1 label 999a6993f0fa5533, channel d7121a34d073edc5, segments f3841d4d940c63e8, z after the sediment pass e0a14921fa9a77e0, equal to main's; peak commit 816 MiB; `MARK lakes` prints; dams at k=1: 516 land pits, 0 deeper than 20 m.
- Not in this change: the k=4 runs (they start from this pushed commit).
