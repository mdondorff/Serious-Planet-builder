## Why
- Lead #166 (owner, 2026-11-18): kept-basin interiors are counted apart from the uphill and agreement bars, in their own columns. A kept basin is water under reading 22, and the gate's routing crosses its bed (SPIKE-132 addendum 5: most uphill steps lie in depressions).
- Lead #166 (owner, 2026-11-18): agreement starts at the graph's own heads. Routed cells above a graph head are not misses of the graph.
- Lead #162 (2026-11-18): missing spill nodes are counted over the box's basins, the margin's apart. On the five windows every missing spill node (28 to 1,392) belongs to a basin whose floor lies in the margin.

## What changes
- BUILD-060: a kept-basins part (`kept_basins`, `kept_cells`, `kept_steps`, `kept_uphill_steps`) and a `RIVER-GATE kept basins` line.
- BUILD-060: the agreement counts kept-interior cells and routed cells above a graph head apart (`agree<i>_routed_kept`, `agree<i>_graph_kept`, `agree<i>_routed_above_head`), outside both shares.
- BUILD-060: `g_basin_spill_missing` and `bar_spill_missing` over basins whose floor lies inside; `g_basin_spill_missing_edge` for the rest.
- BUILD-060: `bar_uphill` over the steps outside kept interiors. The `uphill_pct` column (BUILD-057) is unchanged.

## Impact
cache (`river_gate`, `river_gate_graph`); tests. A row's agreement and uphill bars move on every window with kept basins or graph heads.
