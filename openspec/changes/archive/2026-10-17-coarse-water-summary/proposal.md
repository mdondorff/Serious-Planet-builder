## Why
M5 item 9a.1 (the dive's detail lag): the first wave of the dive is 36 coarse tiles, and each tile's water fill reads up to 25 blocks of the stored bake through a 31-block cache (about 900 loads at about 180 ms, the first tile resident at frame 430, lag mean 1.7 against a budget of 1). The decision of the planner (2026-10-17) is a coarse level of the water field that the bake writes once, not a stopgap in the tile jobs. ADR 0058 (proposed).

## What Changes
- `generators::water_summary` (GEN-024): the summary of a bake (a flag pair and a band elevation per 64 x 64 bake cells), the per-block computation, and the tile query that answers tiles at or above the block level.
- `SeaProducer::with_summary`, `WaterField::coarse_tile_water`, `WaterSource::coarse_tile_water`: `WaterBook::fill` and `WaterTileCache::tile_water` take the coarse answer first and read no block for it (ATMO-012, ATMO-019).
- `cache::global_store` (STRM-025): record kind 5, codec, key (`cpu-water-summary-v1`), the pass in `build_global` that writes only a missing summary, `load_water_summary`, the verification, the line of `generate --global-dir`.
- The viewer's store world loads the summary and refuses a store without it, naming the command.

## Impact
Specs: GEN-024 and STRM-025 added; ATMO-012 and ATMO-019 modified. No `GLOBAL_VERSION`, `MESH_VERSION` or generator version change; no existing key moves. Stored tiles and goldens are unchanged: the in-memory bakes of the golden scenes have one block per face, so no tile is coarse. Existing stores need `generate --global-dir DIR` once (the 16384 store: 44 s, 451,928 bytes).

Expected golden changes: none.
