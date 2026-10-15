## Decisions

- Target = `plan_terrain_cached` of the frame's own request over the case's field: the synchronous plan (no new selection code). Deterministic, so a post-pass is exact.
- Drawn set = tile ids of the plan drawn that frame (`DrawPlan::plan.nodes`): exactly the tiles that are resident and drawn (uploaded tiles only; deferred ones are not in `ManagedLoop::resident`). The deepest drawn ancestor-or-self is the drawn level; a drawn set holds no overlapping tiles.
- Finer than target: clamped to 0. A node with only drawn tiles below it (the stale plan was finer than the new selection) is covered, lag 0.
- Uncovered: no drawn tile at, above or below. Counted as level + 1 (the previous blank rule), reported with its tile id and as a per-frame count, but it is not the hard "blank tile" measure: the stale-plan rule lets a newly revealed edge go undrawn for a frame, and judging that as the hard blank would change what the harness fails on. Open point for the planner.
- Weight: `node_weight` (projected pixels of the tile edge at its origin distance, capped at the screen), unchanged.
- Perf scene: records `FrameDraw { camera: Option<Camera>, drawn: Vec<TileId> }` per frame (bounded: the drawn plan's ids, a few KB). Post-pass after `drop(loop)`, `available_parallelism` scoped threads over contiguous chunks, one field and `SelectionCache` per chunk.
- Verdicts: `FrameRecord::lag` stays `Option<f64>` only so missing data is representable; any `None` in a rule's frames makes that rule OVER (all gates), never "not measured".
