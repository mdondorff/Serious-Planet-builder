## Context
- `region-items` (WDEF-012, GEN-029) cuts the deltas of a layer inside a mode-2 region; the region is filed in the layer it clips and is read by the tiles its reach touches. A mode-1 region cuts everything outside its polygon.
- The tile's read set (ADR 0066 point 2: footprint entries, kind 3, and summary entries, kind 4) is what its key chain covers (WDEF-008, STRM-027, CON-07).

## Decisions (proposed)
1. **Global flag.** `ItemRec` gets `global: bool` (true for a mode-1 region); the item is still filed in its cell. `WorldDefinition` keeps, per body and layer, the set of global item ids.
2. **Footprint.** `footprint_items` (tile form) and `footprint_items_cap` (cap form) return the global items of the layer besides the items by reach; the result order stays (priority, id).
3. **Read records.** A new kind 5, `global items of layer L`: the hashes of the layer's global items in id order (empty layers add nothing, so the read set of a world without mode-1 regions is what it is today). The tile's read set and the root of the summary tree of the layer include it. A tile of any level in the layer then re-keys when a mode-1 region is added, edited or removed; this is the one-off refresh of the layer.
4. **Limit.** One mode-1 region per layer and body; the put of a second is refused naming the first.
5. **Mask.** In `apply_stamps`, a mode-1 region returns the base where the point is outside its polygon (and beyond its reach, a point is outside); inside, nothing is cut.
6. **Bounds.** The range is widened with 0 for the tiles that read a mode-1 region (as for mode 2).
7. **Tests.** The key chain: a tile far from the polygon changes its key with the region; a tile of another layer does not; two edits of the polygon give two keys; the summary root follows. The cut outside; the bounds; the refusal of a second region.

## Risks
- Every tile of the layer is refreshed once per edit of the region (the editor's preview still draws the edit meanwhile). The layer should be a small, authored one.
- The new read kind touches ADR 0066's chain: an ADR note, and a stage version bump.
