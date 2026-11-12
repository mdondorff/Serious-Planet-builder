## Why
The river plan recomputed the signed-distance coverage of every node with a river each frame (about 1.2 s of CPU a frame in the river-flight smoke, 250+ nodes). A node's coverage is a function of its tile, the request and the channels alone (CON-30), so a frame can be a lookup (architect, 2026-11-12: the order is the cache, the atlas, the smoke again, then the perf window).

## What Changes
- PR 1 (this one): `frame::river::RiverCache` (coverage by tile, byte-bounded LRU, negative results kept, stamped with the source's identity, grid, width law, limits and node size), `plan_rivers_cached` (lookup, build on a miss: the same plan as `plan_rivers` bit for bit), `plan_rivers_lookup` (lookup only: the tiles missing and the not-ready count) and `fill_river_cache` (the body of a background fill); `RiverSource::id` (the commit of a reader). `plan_rivers` is the fresh reference, now built from the same per-node function.
- PR 2: the app: the fill on the job system off the frame path, the cache held beside the river source, the plan a lookup in the viewer and the river-flight scene, the not-ready count per frame and in the scene's report.

## Not in this change
- The 2D atlas that replaces the 256-layer array (REND-061, the next change); the perf window.

## Impact
- Code: `frame::river`, `app::rivers`, tests. No key, format or golden moves. Requirement REND-064.
