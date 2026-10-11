## Why

M3 plan and ADR 0007: an edit must invalidate only what it touches. Node reads already do (WDEF-004). Authored items and their spatial queries are the other half: the cell is the invalidation unit, and ADR 0006/0007 require that a query which found nothing is still part of the key.

## What Changes

- `world-def::cells`: cell level per body radius (40 km cells, level 8 at Earth's radius, `libm`), `cell_of`, `cells_covering(tile, level, max)`, `cell_hash` with a fixed hash for the empty cell.
- `WorldDefinition`: authored items by `(body, cell)` as `(id, content hash)` with `put_item` (add or move), `remove_item`, `items_in_cell`; `hash()` covers items (and equals the node hash when there are none), `base_hash()` covers the nodes only. Item payloads, ULIDs, geo-anchors and TOML item files belong to M6 and to the change that adds string nodes.
- `GenContext::query_cell` and `query_tile`: record `derived.cell.<body>.<cell>` with the cell hash, empty cells included, plus the radius read.
- `HeightField::inputs_hash_for(tile)` (default: the field-wide hash): height and mesh keys, and `MeshKeyer`, use it, so a field that queries cells keys each tile by what that tile read. No existing key changed.
- Modified: WDEF-004 (queries), WDEF-005 (now active). Tests: world-def `cell_index` (7), generators `cell_keys` (2: local invalidation of height and mesh keys, add, edit, move, remove).
- Not here: a real stamp generator, the halo (a generator that needs neighbouring cells queries them), coarse tiles reading the whole body (`query_tile` returns `None` above the limit), loading items from files.
