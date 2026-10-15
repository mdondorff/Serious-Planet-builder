## Why

ADR 0033 (accepted in part, points 1-7 and 10) adopts SPIKE-06's compact tile format for levels 3 and finer: 8.8 KB per tile instead of 53.7 KB at cells 32, exact morph targets, two 16-bit octahedral normal components, levels 0-2 and steep finest tiles in the vertex format. M4 item 7 builds it in three changes; this is the first: the GPU-free builder and format, with the tests that pin the format before any renderer uses it.

## What Changes

- generators: `compact` module (GPU-free, CON-02/CON-03). `CompactTile` (texels `[height u16, normal x snorm16, normal y snorm16, reserved]`, a 128-byte record: nine tile-local control points, `hr_min`, `hr_step`, `h_origin`, skirt depth, flip), `compact_tile_field`, the format decision `vertex_reason` / `tile_data_field` (levels 0-2 and a finest tile with more than 65.535 m of relief give the vertex format, CON-13), `TileData` (the two formats), 16-bit octahedral encode/decode, and the CPU mirror of the compact vertex shader (`shade_vertex`), the oracle of the renderer change.
- A tile has one key in either format: the compact tile carries the mesh key, so plans, `MeshKeyer` and the pack are unchanged.
- spec GEN-019 (new).

Not in this change: the renderer (atlas pool, shader, draw runs), the pool/share model, MESH_VERSION (the bump comes with the switch-over, because it changes every mesh key and golden), the finest-level cap (see design).
