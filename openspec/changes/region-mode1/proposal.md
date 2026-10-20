## Why
The owner asked (2026-10-19) that a region's mask can clip a layer's edits inside or outside, like a layer mask. `region-items` implements "clip inside" (mode 2) and refuses "clip outside" (mode 1): everything outside the polygon, anywhere on the body, is cut, so every tile of the layer must read the region, which the footprint (per cell, by reach) does not do. This change is the design of what mode 1 takes; it is not built. The owner decides whether it is worth its cost (opus-planner takes it to the owner).

## What Changes (if accepted)
- **Global items of a layer** in the world definition: a mode-1 region is returned by every footprint query of its layer (tile and cap forms), in addition to the items by reach.
- **A new read-record kind** (5, global items of a layer: their item hashes) in the tile's read set and in the summaries' root hash, so the key of every tile of the layer changes when a mode-1 region is added, edited or removed (CON-07).
- **One mode-1 region per layer and body**: two outside-clips would be the intersection of two polygons; the second is refused.
- **The mask** in `apply_stamps`: a mode-1 region cuts every delta of the layer outside its polygon (GEN-029 extended).
- **Bounds**: widened with 0 for every tile of the layer (as for mode 2).

## Impact
- Specs: WDEF-007/008 (footprint and summaries: the global entry), WDEF-012 (mode 1 allowed), GEN-029 (mode 1). An ADR note: ADR 0066's read-record kinds gain a fifth.
- Cost: about a day of work; every resident tile of the layer is refreshed once per edit of the region (a full refresh of the layer).
- Alternative without engine work: the editor expresses "clip outside" as clip inside several regions that tile the complement within the area of interest.
