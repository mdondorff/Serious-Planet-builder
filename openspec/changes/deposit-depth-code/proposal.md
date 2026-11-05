## Why
Owner decision 2026-11-05 (planner inbox #71 item 2): the deposited depth of kind 15 is a byte of 1 m steps, which cannot resolve the material thresholds of 0.3, 0.5 and 1 m: on seed 14's mountain tile 2/12/10, 18.6 % of the cells carry a deposit and 17 % of those are under 1 m (3.2 % of all cells; 6 % of the deposited cells are under 0.3 m), while the deepest deposit is 429 m (255 m clipped).

## What Changes
- The deposited depth is a square-root code, `round(12 sqrt(m))`, decoded `(code / 12)^2`, 0 to 451.6 m: the step is 0.10 m at 0.3 m, 0.12 m at 0.5 m, 0.17 m at 1 m, 0.53 m at 10 m, 1.7 m at 100 m and 3.5 m at the end; code 12 is 1 m.
- `generators::bake::tile::depth_code` and `depth_m` carry it; the reader (`LayerChannels::depth_m`, `erosion_depth_m`) decodes through the same functions, so no other code changes.
- `CHANNELS_VERSION` 4; the channels' key carries the scale (2, a square root) and the factor 12.
- STRM-072 and GEN-111 amended in place (the m9-bake delta; no new requirement ID). The test of the old 1 m code is replaced by one of the new code (strictly increasing, the steps at the thresholds, the round trip within half a step in the square root).

## Capabilities
### Modified Capabilities
- `generator-pipeline`: GEN-111 (planned, in `m9-bake`)
- `streaming-cache`: STRM-072 (planned, in `m9-bake`)

## Impact
- Size: the compressed plane of tile 2/12/10 (1,754,126 cells with a deposit of 9,437,184) is 2,602,694 bytes as the square-root code against 2,124,377 as the 1 m code (0.276 against 0.225 bytes a cell, +22 %).
- `CHANNELS_VERSION` is in the key of kinds 14 and 17 too: a store from before is refused as a whole, and a layer whose pass-1 store was already dropped needs pass 1 again (the areas are copied from it).
- Code: `generators::bake::tile`, `cache::layer_channels` (key). Tests: `cache/tests/layer_channels.rs`. A store baked before is refused as another key. No golden moves.
