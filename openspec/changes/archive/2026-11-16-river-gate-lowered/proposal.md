## Why
- #160's D-1 table needs "ground lowered more than 50 m below the start surface" for every variant and window, read through one `river-gate --table` build. The gate counted river cells only (the erosion depth).

## What changes
- BUILD-057: a `lowered` line and three fields (`land_cells`, `lowered_cells`, `lowered_pct`): the land cells inside whose surface lies more than 50 m below the start.
- TEST-137: a fixture (60 m everywhere: all; 40 m: none).

## Impact
cache (`river_gate.rs`); tests.
