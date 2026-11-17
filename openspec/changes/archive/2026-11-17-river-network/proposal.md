## Why
- River-graph plan, pre-work (lead #163 item 2): the fine graph's density and hierarchy are judged against a bar from the GLO-30 benchmark, fixed before any RG result.
- The gate had a density at 75 km2 only, no stream hierarchy, and no input for the benchmark's raw grids (SPIKE-119 used an unmerged spike example).

## What changes
- BUILD-059 (new): `river-gate` measures the channel network at two thresholds (`--network-km2 A,B`, default 1 and 75 km2): density, Strahler streams by order, `R_b` over the orders below the highest; it takes `grid=file,w,h,sea_level_m` as input.
- TEST-137: a hand-built network (5 / 2 / 1 streams, `R_b` 2.5) and the grid target.

## Impact
cache (`river_suite.rs` `network`, `river_gate.rs`, the `river_gate` example), xtask (`rivergate.rs`, help); results `spikes/river-suite/results/network-bar.md`.
