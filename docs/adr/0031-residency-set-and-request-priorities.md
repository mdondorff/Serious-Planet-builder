# 0031 Residency set and request priorities
Status: proposed (sketch by opus-planner, 2026-10-13, from the owner's decisions of 2026-10-13; sonnet-explorer completes it with SPIKE-05's counts, M4 item 4)
Date: 2026-10-13
Source: owner's decisions 2026-10-13 (`docs/status.md`, M4 queue (c)); architecture report §4 and the M4 row; STRM-002, STRM-005, STRM-007, LOD-007; constitution CON-03, CON-04, CON-20, CON-22

## Context
Today the streamer wants exactly the nodes the frame draws, and that selection is cut to the view frustum (LOD-007). Looking around is therefore a cache miss: rotating the camera requests tiles that the position alone already determined. The owner's requirement: rotation (mouse look) must be close to free and must never fetch tiles that were not wanted before; a teleport must show something at once and then fill in everything the new position needs, not only the current view; prediction from the camera's velocity cannot be relied on (a teleport has none, a mouse is erratic).

## Options
1. **Frustum-driven residency with a margin** (today plus a padded frustum). Fewest tiles. Rotation beyond the margin still misses; the margin is a guess.
2. **Position-driven residency** (chosen below): the wanted set is the distance-based LOD selection around the camera in all directions, cut only by the horizon. The frustum cuts the draw set only. More resident tiles (by hand roughly 3-6x the near tiles at ground and aerial heights, not measured: SPIKE-05 measures it); rotation is free.
3. **Velocity-predicted residency**. Good on smooth paths only; wrong for teleports and mouse look. Kept only as an extra.

## Decision (proposed)
1. **Two sets.** The *residency set* is a pure function of the camera position and the LOD parameters (LOD-008 distances, horizon cut, no frustum cut). The *draw set* is the residency set's selection cut to the frustum (LOD-007), so a frame draws only resident tiles, plus the parent fallback of STRM-002. Both are computed in GPU-free crates (CON-03).
2. **The view direction orders requests and never decides whether a tile is wanted.** Request order, highest first:
   1. missing nodes of the draw set, parents before children (never blank, STRM-002);
   2. the rest of the residency set, coarse before fine, nearer before farther;
   3. prefetch: the residency set at the predicted position along the direction of travel (translation only; rotation is never extrapolated), only while 1 and 2 are empty.
   Screen-space error times visibility (STRM-005) orders within class 1.
3. **Cancellation** (STRM-005): a queued or in-flight request that is in no class any more is cancelled; prefetch requests are cancelled first when the travel direction changes.
4. **When the budget is too small for the residency set** (ADR 0033's terrain share): keep the draw set at full detail and the root level resident (STRM-007); degrade the rest of the residency set from the outside in (finest levels of the farthest nodes first), never the draw set. The share therefore bounds the surroundings, not the view.
5. **Teleport** is a camera move with no velocity: class 1 fills first (coarse at once from the resident roots, sharpening), class 2 follows. Its detail-lag budget is defined in ADR 0032.
6. All of it runs on the injected clock, I/O and spawner (CON-04) so the scheduler is tested at tier A with fakes.

## Consequences
- Rotation needs no generation and no upload when the residency set fits the share: testable at tier A (a 360° yaw at a fixed position requests nothing).
- VRAM and generation work grow with the surroundings: ADR 0033 sets the share, SPIKE-05/06/07 decide whether the surroundings fit at full detail.
- STRM-005's ordering text gains the three classes (spec delta); LOD-007 stays the draw-set rule.
- Tests: tier A for the yaw case, the teleport fill order, cancellation on a travel change, outside-in degradation under a small share; perf scenes per ADR 0032.
- Open for the completion: the residency set's own horizon padding; whether class 2 uses the draw set's LOD thresholds or a coarser `tau` (fewer tiles, softer surroundings after a turn).

## Licences of adopted code or techniques
None.
