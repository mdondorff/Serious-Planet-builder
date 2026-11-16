## Why
- The corridor bar (#159) read the start's 10 x 10 relief over a store's counted tiles. On R1h that gave 30.3 m, not the 27.5 m P3 recorded before the results, which is the whole-window value. Lead, 2026-11-16: use the recorded 27.5 m.

## What changes
- BUILD-057: the corridor bar reads the start relief over the region's box, so a store's tiles do not move it. Window mode is unchanged: its counted cells are the box. Only R1h's bar changes: the other windows sit at the 10 m or 50 m clamps either way.
- TEST-137: a fixture where the counted cells and the box differ.

## Impact
cache (`river_gate.rs` `measure`); tests.
