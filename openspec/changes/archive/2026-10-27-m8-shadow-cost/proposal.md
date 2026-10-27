## Why
M8 criterion 5 (the horizon shadows and the material map together at most 1.5 ms of terrain-pass GPU time at 1440p) has no measurement: the lit scene times the terrain pass but no scene turns a feature off, and it draws no horizon maps (opus-planner, 2026-10-27).

## What Changes
- Perf scene `shadow-cost` (`cargo xtask perf stream shadow-cost --store DIR --global-res N --poses FILE --out-dir DIR`): the settled poses of the pose stills at 1440p, the terrain pass timed in four variants (full; without the horizon maps; with the ground detail at strength 0; both), medians of 41 waited frames; log `perf/logs/shadow-cost.json`.
- The shares are differences of variants. The shadow share under-reads by the branch the shader still takes with every node on slot -1; the material share over-reads by the cost of the detail at strength 0. A pose within 20 % of the budget is flagged. The scene also times the pass with the code compiled out of the shader (`MeasureVariant`, a measurement build of the same source; no product shader change), which has no branch left and is what the shares and the verdict read.

## Impact
- Spec: REND-044; tier A test of the shares, the budget verdict and the option checks.
- Code: `app/stills.rs`, `app/lib.rs`, `app/streamperf.rs`, `xtask/src/testcmd.rs`.
