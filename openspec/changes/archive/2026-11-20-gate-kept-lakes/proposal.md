## Why
- The owner chose option C (2026-11-20, reading 37): `--kept-drained exit` keeps the kept L basins whose rims no line leaves below the spill.
- Those basins are lakes by the graph's contract, so the gate must measure them apart from the closed basins (ponds) it reports.
- The owner's conditions (2026-11-20):
  - BUILD-055's raw closed-basin share stays as it is;
  - the ponds and the kept river length get their own bars;
  - a kept basin whose rim was lowered below its recorded spill is a graph violation.

## What changes
- BUILD-060, on a row with a graph:
  - `ponds_cells` and `ponds_pct`: river cells in closed basins outside the kept-basin interiors, over the river cells outside them. The interiors are BUILD-060's existing mask.
  - `kept_river_cells` and `kept_river_pct`: river cells in the interiors, over all river cells.
  - `kept_lake_cells` and `kept_lake_km2`: the interiors' area.
  - `g_spill_mismatch`: kept basins whose depression level at the floor lies more than 0.5 m under the recorded spill.
- Bars:
  - `bar_ponds` <= 10 %;
  - `bar_kept_river` <= 5 %, 10 % on M1;
  - `bar_violations` adds `g_spill_mismatch`.
- A `RIVER-GATE kept lakes` line after the basins line.
- BUILD-055's `basin_cells` and `basin_pct` are unchanged on every row; a row without a graph gets `-` in the new columns and no new bar.

## Expected golden changes:
- none: the gate is a CPU measurement; no rendered output changes.

## Impact
- Crate: cache (`river_gate.rs`, the `river_gate` example), with a tier A test in `river_gate_graph.rs`.
- Rows with a graph get two more bars, and `bar_violations` can fail on a spill mismatch.
