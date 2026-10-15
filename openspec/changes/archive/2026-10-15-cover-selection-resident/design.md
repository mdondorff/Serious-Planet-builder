## Cover rule

`cover_selection_bounded(map, camera, p, resident, bounds)`: walk from the resident roots that pass the cull. For tile `t`: it descends iff `level < max_level`, the distance from the camera to its volume (when `bounds` holds it; otherwise to its resident origin less its radius, the old estimate) is below the level's split distance, and every child that is in view is resident. In view = `!culled(c)`: beyond the horizon (`below_horizon_at`), outside the frustum with the padded reference sphere, outside the frustum with the tile's own volume plus skirt depth (the tight cut, LOD-007/008). When the bounds are the bounds of R for exactly the camera position and the LOD parameters (`TileBounds::complete_for`), a tile that is not in R also counts as culled: R (position only, balanced) holds every tile the plan of any view direction can select or descend through, so a tile outside it has no selected tile under it (its children lie beyond the horizon). This is what lets a rotation at the settled state equal the plan: without it a non-resident, horizon-dead child blocks its parent. With a different position (a moving camera) the old behaviour stands: a non-resident child that is not culled blocks the descent (the coarse parent is drawn, never blank).

The leaves are balanced with `balance_leaves` (the plan's 2:1 rule); a leaf of the balanced set that is not resident is replaced by its nearest resident ancestor (undoing that split) and finer tiles under a stand-in are dropped. The result is a subset of the resident tiles.

Where the bounds come from: `residency_set_field` asks the selection cache (the one the plan used) for the geometry and volume of every tile of R, which the selection has all visited, so it is lookups. The worker delivers them with R; the frame reads them (`PlanPipeline::begin_frame` passes the newest R's bounds). A tile missing from the bounds is judged by geometry alone, computed on the fly (looser cull, never tighter than the plan).

## Drawing rule

`draw_plan_with`: with a base plan made for exactly the request, as before (Exact, with stand-ins for non-resident nodes). With a base plan made for another camera (within the 0.5 degree / quarter split distance limit of `plan_usable`), the stand-in plan is drawn only if `plan_covers_view` holds, else the frame draws the cover. Without a base: the cover.

`plan_covers_view(map, camera, p, bounds, view_cover, drawn)`: `view_cover` is the cover for the frame's camera. A stack walk: a tile that is drawn is covered; a tile that has drawn tiles below it (an ancestor of a drawn tile) is covered iff all its children that are in the frame's view are covered, recursively; a tile with a drawn ancestor (the plan is coarser) or with nothing drawn at, above or below it (a sliver turned into view) is not. Conservative: false positives only send the frame to the cover, which is correct when R is resident. Cost: a walk over the cover (a few thousand hash lookups with cached bounds), done on frames whose plan is stale only.

## Why uncovered gates

ADR 0032's hard measure is "a tile, root or ancestor, for every visible node". The uncovered figure of #152 is exactly that, measured against the fresh selection. With the cover rule above it is 0 in every frame of the fakes' dive, teleport and rotation scripts. `dive-fast` stays report-only (owner's amended criteria).

## Risks

- The cover is computed on every frame whose plan is stale (not Exact): cost is bounded by the visited tiles with cached bounds; measured only by the architect's perf window.
- A frame can alternate between a stale plan (morphing) and the cover (no morph) while turning near the limit; the cover has no morph (a tile drawn as a stand-in or cover does not morph, as before).

## Declaration

Gating uncovered nodes (every scene but `dive-fast`) is the reading of ADR 0032's hard measure ("a tile, root or ancestor, for every visible node") decided by opus-planner under the owner's delegation, 2026-10-15; `dive-fast` stays report-only per the owner's amended criteria in docs/status.md. The bounds of a degraded R (ADR 0031 point 4b) are those of the full R, so the "tile not in R" rule holds for it too.
