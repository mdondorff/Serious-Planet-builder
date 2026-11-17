## Why
- SPIKE-137's gap 1 (lead #166a): river-gate measured a window's start surface only, so G2 could not measure the RG chain's final surface.

## What changes
- BUILD-060: `--surface FILE` (a candidate's surface on the window's L grid, little-endian f64, row-major, finite) is measured against the window's start surface; not with `--store` or `--grid`; a table row takes it as `#surface=`.
- TEST-137: the surface reader's refusals and the row suffix.

## Impact
cache (`river_gate_graph::read_surface`, the `river_gate` example), xtask (`rivergate.rs`, help); tests.
