## Why

Part 2 of 3 of the compact tile format (ADR 0033 points 3-5): the renderer side, built on the GPU-free builder of `compact-tile-builder` (GEN-019). The default render path does not change here (a compact renderer is opt-in); the switch-over, the residency wiring, MESH_VERSION and the goldens are part 3.

## What Changes

- render: `TerrainRenderer::with_compact_pools` / `new_compact` / `with_frames_in_flight_compact`: atlas pool of `Rgba16Uint` textures (one block = one 4096 x 4096 atlas, 124 x 124 = 15,376 slots at cells 32), a persistent 128-byte record buffer indexed by slot, the compact vertex shader (vertex pulling from texels, exact morph target, skirts; the product fragment stage spliced in), the vertex pool kept for levels 0-2 and a fallback region for CON-13 fallback tiles. `upload_tile` / `upload_tile_at` take either format (`TileData`); `upload` / `upload_at` still take meshes (a mesh of level 3 or finer in a compact renderer is a fallback tile).
- Drawing: one frame draws both formats. The indirect multi-draw's runs gain `DrawBuffer::Atlas(n)`: consecutive nodes in one atlas form a run drawn with the compact pipeline, other runs with the vertex pipeline, in the plan's order; the per-draw loop is the reference and agrees. `first_instance` stays the node index; the node's tile reference (16 B, per frame) carries the slot's texel origin and slot index.
- frame: `pool::compact_format_pool(_with)` / `tier_compact_pool` (15 atlases = 230,640 slots mid, 23 = 353,648 high, at cells 32); `draw::build_draw_list_placed`.
- streaming: `PoolConfig` gains `block_alloc_bytes`, `slot_record_bytes`, `fallback_slots` (zero for the vertex format, so its numbers do not change); the slot records count with the fixed items, the manager's vertex pool excludes the fallback region.
- spec REND-016 (new).

Not in this change: the default renderer, the apps and `--stream` (part 3), MESH_VERSION, goldens, headroom pins, finest level 22.
