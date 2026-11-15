## Why
- P3 E1 (lead #151, owner: P3 rewritten after the external review): the river gate counted basins, cuts and relief but not the shape of the rivers and valleys; the candidates and the Earth benchmark (spikes #121) need the shape metrics on one definition.

## What changes
- BUILD-057: `cargo xtask river-gate` prints nine more lines, one per metric: gradient, valley width, asymmetry, drainage density, catchment ratio at H1 crossings, seam continuity at the region's edge, narrowness, slope-area scaling, relief power spectrum. The same code runs on a window and on a store (the baked ground).
- TEST-137: tier-A fixtures for each metric.
- BUILD-055: two more canonical windows, M1 (mountain belt) and F1 (low-gradient river landscape), from SPIKE-119 (spikes, 2026-11-15).
- Every count line already carries its absolute count; the new lines do too.

## Impact
cache (`river_suite.rs`, `river_gate.rs`, tests). The example is unchanged (the lines come from `GateReport::lines`); xtask gains the two window names.
