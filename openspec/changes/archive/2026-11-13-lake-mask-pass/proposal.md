# lake-mask-pass

Why: E1 (P1): after PR 4a a tile that holds a lake cell takes the lake's level for its whole grid, so the water pass floods every hollow under the level inside the tile (the one-level-a-tile limit of `frame/src/water.rs`). Item 15 PR 4b (`openspec/changes/runtime-composition/design.md` section 15): the pass draws a lake node only where the commit's lake mask is wet.

What: the water plan carries a lake mask per lake node (`WaterMask`: the node's `(cells + 1)^2` samples read at their nearest cell of the commit's lake grid, 255 wet, 0 dry; memoised per tile and body in `WaterTileCache`); a lake node none of whose samples is the lake's carries no water; the renderer writes the masks into the river coverage atlas's slots after the rivers' (REND-061 layout) and puts the slot word into the reserved fourth word of the water node record; the water fragment discards where the interpolated mask is under one half. The sea has no mask and draws as before (its goldens and known answers are unchanged). The masks are part of the plan's text form (`wmask` lines; a plan without lakes writes the old text).

Decisions: nearest-cell sampling at every LOD (architect, 2026-11-11, decision 3: a conservative OR-mip replaces it if the flight shows popping lakes); a mask slot past the atlas leaves its node drawn without the mask, counted in the frame stats (`lake_masks_dropped`, never silent).

Expected golden changes:
- water_class_lake_mask_stripes: new (first golden of the lake mask; the owner's /bless, ADR 0017).

Requirement IDs: REND-065, TEST-133 (lead, 2026-11-13). Source: architect 2026-11-11 (PR 4 decisions), lead 2026-11-13 (E1: the golden stays pending, nobody waits on the bless).
