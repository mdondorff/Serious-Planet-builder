## Why
- The owner chose option C (2026-11-20, reading 37): `--kept-drained exit` keeps the kept L basins whose rims no line leaves below the spill.
- Those basins are lakes by the graph's contract, but the gate's closed-basin bar (BUILD-055, at most 10 %) counted the river length through them as closed basins.
- With exit, that moved R1h and F1 over the bar (11.41 % and 10.12 %).

## What changes
- BUILD-060: on a row with a graph, the closed-basin share counts river cells outside the kept-basin interiors. The interiors are BUILD-060's existing mask: the depressions of the gate's basin flood that hold a `protected_surface` basin's floor.
- New columns:
  - `basin_all_cells` and `basin_all_pct`: every closed basin, the former form;
  - `kept_lake_river_cells` and `kept_lake_river_pct`: river cells in the kept lakes;
  - `kept_lake_cells` and `kept_lake_km2`: the kept lakes' area inside.
- The basins line names the split.
- A row without a graph is unchanged: every closed basin counts, and the kept columns read `-`.

## Expected golden changes:
- none: the gate is a CPU measurement; no rendered output changes.

## Impact
- Crate: cache (`river_gate.rs`), with a tier A test in `river_gate_graph.rs`.
- Rows with a graph move their `basin_pct`; the former value is `basin_all_pct`.
