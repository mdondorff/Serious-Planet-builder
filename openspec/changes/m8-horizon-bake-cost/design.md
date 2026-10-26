## March
`march_steps` makes `(d, d^2 / (2 R))` for every step of the recurrence `d += step_min + growth * d` once per bake. The march of a texel and azimuth walks the table; each double is the one the old loop computed (same operations, same order), so the angle and the byte are unchanged. `libm::floor` of the sample position becomes an integer floor (exact). Divisions `d * dir / texel` and `rise / d` stay as they were (a multiplication by a reciprocal would change bits).

Max-mip skip was measured and dropped: with an exact test it skipped 60 % of the steps and gained nothing (6.5 to 8.7 s against 6.8 s).

## Height tiles
A bake needs the heights of the cells `[x0, x0 + n + 2 halo)` on its chunk's face plane; with `halo <= n` these lie in the nine blocks around the chunk. Blocks inside the face are cached; blocks past a face edge (the extended plane, rotated against the neighbour's own) are evaluated each time and not kept. A larger halo evaluates the whole raster as before.

Key of a tile (everything its heights depend on):
| Part | Source |
|---|---|
| Code and lattice | `BAKE_VERSION`, `ANET_VERSION`, `CHUNK_STAGE_VERSION`, cells per chunk, radius |
| Baked input | the world's bake key |
| Definition apart from items | `base_hash()` (nodes, height field), chunk stage position per layer of `LAYER_STACK` |
| Coupled lens | lens id and whether the run is ready (the field chain does not hash it into a tile key) |
| Items | hash of the items touching the block with the stamp stage's halo (`STAGE_HALO`), the same footprint query as the horizon's edit word |
| Chunk | the hash of the block's ready result (0 if not ready), read before and after the field is built; a block whose result changed in between is used, not kept |
A neighbour's edit changes the item hash of the blocks it touches, so those are evaluated again and the rest are read.

Limit: a custom (non-`LAYER_STACK`) layer name with a changed chunk stage position is not in the key; the world has none today.

## RAM (ADR 0083 point 4)
64 tiles x 614 x 614 x 4 B = 96.5 MB at the A-net default, plus the raster of a running bake (1824^2 x 4 B = 13.3 MB each, at most `workers` at once). The cache evicts the least recently used tile. `HEIGHT_TILES_MAX` is the one number; `with_tile_cap(0)` turns the cache off.
