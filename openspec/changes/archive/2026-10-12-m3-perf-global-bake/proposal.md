## Why

M3 follow-up (m): GEN-014 (the 1 km global bake in at most ten minutes within the memory budget) is measured by the performance harness through the architect, never by a coder (CON-20). The harness had no scene for it.

## What Changes

- `test-render --perf --scene global-bake` (`crates/app/src/globalperf.rs`): reference-adapter check and the ADR 0020 readiness gate, then `build_global` at `--global-res` (default 16384) into `target/perf/global-bake`; prints and logs wall time, blocks, the most blocks alive and their bytes (derived from the block counter), store size and budget failures to `perf/logs/global-bake.json`; fails when the 600 s or 8 GB budget is violated.
- `cargo xtask perf global-bake [--global-res N] [--threads N] [--machine-ready]` runs it. The existing `perf` is unchanged.
- Tier-A tests: the budget function (boundaries, NaN never passes) and the xtask argument handling. GEN-014's text names the entry. Not run here: the architect measures it.
