## Decisions

- The memo lives in `SelectionCache`, not in a new type: callers already keep one per renderer, and `prepare` already derives the invalidation key (face mapping, implementation id, version, seed, inputs hash, body, radius) that also governs origin height. Mesh keys additionally depend on `cells`, kept beside the memo; a change clears only the key memo.
- No key derivation changes: cached values are exactly what `MeshKeyer::key` and `field.height` returned, so plans, snapshots and `summary` hashes are bit-identical (the tests compare warm, cold and fresh-cache plans).
- Staleness: a `HeightField` whose heights change must change one of the key parts (already the contract for the geometry memo). Same contract now covers origin heights.
- Size: the memo is bounded by the existing `MAX_TILES` rule (the cache starts over past it).
- Cold plans are unchanged by design: the first plan of a tile still evaluates the field once per node. Over the store the cost of a cold altitude plan is block decoding (7b).
- Numerics review fixes: a field that `reads_cells` declares reads per tile at plan time (WDEF-005), so an edit changes keys and heights under an unchanged identity; nothing is reused for it. The cache key gains the reference surface id (the mesh key chains it). A non-finite origin height (a failed bake read) is never kept.
