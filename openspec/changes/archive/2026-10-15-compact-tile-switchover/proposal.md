## Why

Part 3 of 3 of the compact tile format (ADR 0033 points 3-5, 10): the product paths build and draw the compact format by default, the manager's slots are compact slots, MESH_VERSION is bumped, and the goldens drawn from compact tiles go through golden-review. The builder (GEN-019) and the renderer (REND-016) are already on main; the vertex format stays as the reference (`--tile-format vertex`).

## What Changes

- generators: `MESH_VERSION` 5 (the compact construction is covered by it; vertex output is unchanged, so the vertex known-answer hashes are unchanged and only the mesh keys, the plan snapshots that contain them and the compact known answers move). `RenderTile` (renamed from `TileData`, which the height-tile type already uses) gains `origin()`.
- render: `execute_terrain_tiles` (the one-shot path for either format, pools sized to the plan), `TileFormat` and the context's `--tile-format` preference, `TerrainRenderer::for_context`.
- frame: the managed loop's streamer and sink carry `RenderTile`; its pools come from `tier_compact_pool`.
- app: `--tile-format vertex|compact` (default compact) for every terrain path (single view, `--stream`, `--pipelined`, `--flight`, `--perf`, repro replay); the flight, the perf scene and the pipelined loop use the context's format; `MeshSource` builds tiles with the request's finest level, the LOD cap `max_level` (the CON-13 guard reads "finest level" as the cap, ADR 0033 points 5 and 6; a script refuses requests that change cells or the cap, because a resident tile is reused by its mesh key, which does not say which cap its format was decided for).
- streaming: with many small slots per atlas (cells 16) the pool takes fewer atlases so the slot records stay inside the fixed reserve.
- tests: gpu_terrain draws from compact tiles (the goldens), the projection and morph oracles read CPU vertices and judge the compact image; flight compact against vertex; headroom pins from `tier_compact_pool`.
- specs: TEST-011 (upload bytes in the tile format), STRM-006 and STRM-007 (compact pools, slot records, fallback region, headroom), GEN-010 (mesh version 5, pack payload note), REND-016 (default path, flight).

Pack payload: the tile pack stores height tiles (kind height, STRM-009), not meshes or compact tiles; they are unchanged and the pack needs no version bump. A persisted compact tile (ADR 0033's pack delta) does not exist because tiles are regenerated from the field; its codec is not built. Persistent mesh keys change with MESH_VERSION (no pack stores them); repro bundles of mesh version 4 are refused with both versions named.

Each golden below may move at most within rounding or the declared model error; unchanged ones are listed because the product path now draws them from compact tiles wherever they have tiles of level 3 and finer.

Expected golden changes:
- terrain_orbit_tile_id: drawn from compact tiles, rounding at most
- terrain_aerial20km_level: drawn from compact tiles, rounding at most
- terrain_aerial200km_height: drawn from compact tiles (contour pixels move by a fraction of a pixel)
- terrain_ground1m_depth: drawn from compact tiles, rounding at most
- terrain_low2km_morph: drawn from compact tiles, rounding at most
- terrain_cube_corner_normals: drawn from compact tiles (16-bit normals, delta 1)

Not in this change: the finest-level cap 22 (a separate LOD change), the detail term (deferred by the owner), prefetch, the upload cap.
