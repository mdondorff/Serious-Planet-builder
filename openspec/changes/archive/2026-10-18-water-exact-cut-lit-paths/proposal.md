## Why
The window-10 `perf lit` run showed the exact dry-tile cut (#286) without effect in the lit scenes: node counts equal the loose cut's. Root cause: the held views and flights build their meshes with `upload_missing_meshes`, which never ran the water step of a tile job, so the `WaterBook` held neither the answer nor the minimum and the plan fell back to the loose bound. The viewer and the streaming scenes load through `MeshSource` and were not affected (their plans can precede a job's minimum for a frame; such nodes keep the loose verdict).

## What Changes
- `upload_missing_meshes` takes the world and fills the water of each tile it builds, as a job does (no format or data change).
- `WaterTileCache::no_minimum` counts nodes kept with no minimum; the held views log the cut counters.
- ATMO-020: the counter, the requirement that every planning path leaves the minimum, and a scenario.

## Impact
Plan data and a scene setup only. No MESH_VERSION, stored-data or shader change. No ADR.
