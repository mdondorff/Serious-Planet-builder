## Why
- The RGv0 pipeline description (section 25) defines a spill mismatch by two tests: (a) a rim vertex bed more than 0.5 m under the spill, (b) the depression level more than 0.5 m under it.
- BUILD-060 counts only (b); a line the graph draws below a kept basin's spill at its rim contradicts the basin while the surface still holds the water (lead, 2026-11-21).

## What changes
- BUILD-060, on a row with a graph: `g_spill_rim`, the kept basins (floor in the window) whose rim holds an L vertex with its bed more than 0.5 m under the recorded spill.
  - The rim: the cells outside the floor's depression (BUILD-057's basin flood) that touch it, 8-connected.
  - `bar_violations` adds `g_spill_rim` beside `g_spill_mismatch`.
- The `RIVER-GATE kept lakes` line names both tests.
- A row without a graph prints `-`.

## Expected golden changes:
- none: the gate is a CPU measurement; no rendered output changes.

## Impact
- Crate: cache (`river_gate.rs`, `river_gate_graph.rs::rim_mismatch`), with the tier A test in `river_gate_graph.rs` extended.
- Rows whose graph runs a line below a kept basin's spill at its rim now fail `bar_violations`.
