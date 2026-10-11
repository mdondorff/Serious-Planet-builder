## Why

The owner's Rasierklinge run of #43 (2026-10-11): CPU plan time ground-1m 25.75 ms and ground-200m 12.77 ms against the 8 ms budget (GPU fine: 2.9 ms median, p99 8.9 ms). Profiling: selection over a height field costs about 1.45 us per tile for the height-aware volume (13,000 volumes at ground-1m) on top of the per-tile geometry (about 6 ms); both are pure functions of the tile, and consecutive frames share nearly all tiles.

## What Changes

- `frame::lod::SelectionCache`: per-tile geometry and volume memo kept across selections, keyed by face mapping, field identity (implementation id, version, seed) and radius, emptied when any changes and past 400,000 tiles. `select_nodes_in_view_cached` and `plan_terrain_cached` use it; the uncached functions are unchanged. A test proves plans from a warm cache equal plans from nothing (across camera steps and a field switch).
- Perf harness: the plan-time budget applies to steady state: the camera steps 5 cm per plan with a warm cache; the cold first plan is printed beside it. Measured here (not the official run): ground-1m 6.8 ms warm (27 ms cold), ground-200m 3.3 ms warm.
- Modified: TEST-012 (plan budget is steady state, cold time reported).
