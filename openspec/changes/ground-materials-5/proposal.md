## Why
- I8a slice 2a (planner, 2026-11-04): SPIKE-100 D1 (five-material mix) and D2 (six detail octaves per material), slope inputs only; the channels follow in slice 2b.

## What Changes
- The material table grows by `scree` and `wet` (slots 4 and 5; `sand` stays: desert ground exists today); both are off in the Earth default curves, so no picture moves until a look turns them on.
- The shader's per-material coefficients move from constants into the look file (`ground.<material>` octave amplitudes and tilts), so a look can differ per material.
- Six octaves (50 m, 12 m, 3 m, 0.75 m, 0.2 m, 3 cm) behind the look: a look with the old four-octave coefficients draws the old picture bit for bit.
- Look file schema 7.

## Impact
- Code: `frame` (look), `render` (terrain shader, globals), `app` (poses `[look]`). Goldens: none move with the Earth default.
- Cost: measured with `shadow-cost` after the PR, against REND-034's 1.0 ms.
