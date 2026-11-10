## Why
Item 15 PR 2b (architect D16, design notes section 9): the local change L of the new chain needs its stored format. L is the same kind-13 record as the previous layer with layer version 3 and a new codec byte (3); the counts stream is the one item 14 measured (GEN-156).

## What Changes
- `cache::layer_l` (new): `LTileSpec` (tile grid, K7's parameters including the river width law's `a` and `b` (GEN-165), `base_id`, `inputs_id`), `l_tile_key` (implementation id `layer-l-tile`, so the previous layer's keys never collide), `l_tile_record`, `load_l_tile`. Payload `codec 3 | tile | stats (K7: land, burned, clamped, pit cells, pit p50, p99, max) | key | brotli counts`; no fill channel; a reserved, empty water section (length u32 and opaque bytes, at most 16 MiB, not in the key) for the lake water mask the bake writer adds (architect D17, D18).
- `TILE_LAYER_VERSION` 2 to 3 (history line): the previous layer's keys change; its stores are refused, not read (owner 2026-11-05).
- A record of the previous layer in the tile's slot is refused naming both versions (codec 2 and layer version 2 against codec 3 and layer version 3); a tile under another key (another base, other parameters or kernels) is refused, not mixed; a damaged record is an error naming what was found.
- STRM-081 added (streaming-cache); TEST-108 continues.
- Not here: the bake's writer of L tiles from K7, the read in the tile source (the sampler switch), the `commit_id` read with the basin table (A3 section 7.4), the layer manifest's version line for L.

## Impact
- Code: `planet-cache` (`layer_l`, `layer_tiles` version and two `pub(crate)`), tests `cache/tests/layer_l` (3). The previous layer's code is unchanged and still writes codec 2 under its own implementation id (retired with the sampler switch).
