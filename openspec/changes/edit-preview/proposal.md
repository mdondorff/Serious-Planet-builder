## Why
M6 plan row E3: after an edit the exact tiles take up to 300 ms (150 m stamps) to arrive. Until then the stale tiles are drawn as they are (REND-032, stale while revalidating) and the edit is invisible. SPIKE-13 measured a vertex-stage displacement at 4 to 14 ms; the target is a preview of at most 16 ms.

## What Changes
- **Preview stamps** (REND-033): a short list (at most 8) of signed stamp deltas, computed GPU-free from the items before and after an edit (`frame::preview_delta`): an added item is +1, a removed one -1, a changed one both. Each carries its kind, anchor, reach, parameter and bound, as the footprint query returns them.
- **Which nodes** (REND-033): a node of the draw plan is previewed when its resident tile is stale (its mesh key differs from the plan's, REND-032) and a preview stamp's reach touches the tile's cap. A node whose exact tile arrives has the plan's key and is no longer previewed.
- **Where** (REND-033): the compact vertex stage adds the delta of each preview stamp to the height of a previewed node's vertex, along the tile's up, in the order of the list; the profiles are those of GEN-026 on the planar distance in tile-local arithmetic (CON-11). The normal is corrected by the radial slope of the summed delta. With no preview stamp the stage is not entered: every golden and recorded frame is bit-identical.
- **Interface**: `PlanPipeline::set_preview(Vec<PreviewStamp>)`; the draw plan (`DrawPlan.plan.preview`) carries the list and the flags to the renderer. The caller (the editor layer, after `WorldTerrain::set_definition` and `ManagedLoop::edit`) sets it and clears it when the exact tiles have arrived (the flags clear by themselves).

## Impact
- Specs: REND-033 (added, renderer).
- Code: `frame` (`PreviewStamp`, `preview_delta`, the flag in `draw_plan_with`, `TerrainDraw.preview`, `TerrainPlan.preview`), `render` (globals table, node record word, compact vertex stage).
- Goldens: none moves (the stage is skipped with an empty list). A tier C test draws a previewed scene and writes images to `target/review/preview/`; its frame hash is recorded per adapter family, no PNG golden.
- Fidelity: indicative. The preview is the new profile on top of the stale tile (planar distance, no tile-edge continuity beyond the tiles the plan names); the exact tile replaces it.
- Perf: preview <= 16 ms from edit to first frame (SPIKE-13: 4 to 14 ms): measured in a window with the architect's go, not here.
- numerics-reviewer: the shader arithmetic; spec-guardian before archive.
