# Change: ground-detail-cost

## Why
- Perf window 24 (`docs/perf/window-24.md`): the ground detail cost +2.4 ms (1.8 m) and +2.9 ms (0.5 m) of terrain-pass GPU time at 1440p against the budget of 1.0 ms, an M7 exit criterion (opus-planner, 2026-10-22: not accepted).

## What changes (indicative same-process timings on the RTX with `ground_bench`, terrain pass only, 2,383 and 2,276 nodes: detail off 1.17 ms; detail on 2.1 to 2.2 ms before, 1.77 and 1.88 ms after: +2.1 and +2.5 ms to +0.6 and +0.7 ms)
| Step | Detail on at 0.5 m (ms; off 1.17) | Look |
|---|---|---|
| Before (window 24's shader) | 3.65 | |
| The octaves unrolled with constant arguments (no per-octave arrays in local memory) | 3.39 | none |
| The material weights without arrays indexed by a loop counter (constant indices, pairs as vec2) | 2.78 | none |
| A short integer finaliser, the corner products shared, unused table words skipped by a uniform branch | 2.38 | none visible |
| The detail summed octave by octave with the materials' coefficients blended by the weights (no per-octave results kept; the grass tint linearised) | 2.30 | the grass tint a little different |
| Planar noise on the lattice plane the normal lies closest to (4 corners, not 8), the lattice of every other octave turned by 45 degrees | 2.10 | similar; a little more regular grain |
| Four octaves (16 m, 2 m, 25 cm, 3 cm) instead of six (32 m, 8 m, 2 m, 0.5 m, 12.5 cm, 3 cm); the fade starts earlier (0.06 to 0.25 of a cell) | 1.87 | fewer scales: the far mid-scale patches are 16 m, not 8 m and 32 m |

- The experiment knobs were measured, not kept. The look tables (`gloss`, material curves) and the plan are unchanged.
- `ground_bench` (an example): the lit terrain pass of the ground at 1.8 m and 0.5 m with the detail off and on, from GPU timestamps in one process (`PLANET_ADAPTER=hardware`); indicative, no gate.

## Not measured
- The window figure (the bench is terrain only, the window adds the atmosphere and runs gated): one confirming window after this merges.

## Tests
- REND-034 tier A (the anchor of the node record, shifts 9, 6, 3, 0) and tier C (off is inert, on changes the ground, the table drives the picture, other slots give the same picture) run as before.

Expected golden changes: none (the strength is 0 in every plan the goldens use).
