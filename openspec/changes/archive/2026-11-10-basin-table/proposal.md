## Why
Item 15 PR 2a (architect, 2026-11-10; design notes section 9): the lake records of the producer (GEN-163) are bake output and the runtime only reads them. They are stored as a basin table, a new cache kind, not inside the tile records of kind 13.

## What Changes
- `cache::basin_table` (new) and `Kind::BasinTable = 19`, `PARAM_BASIN_TABLE`: one record per cube face, the lakes sorted by basin id (id, floor and spill ids, floor and spill levels, level, `q_in`, `q_out`, area, cells, class and flags, the tiles that hold the lake water), keyed by the table version, the producer version, the hydrology parameters, the tile grid, the base id and `inputs_id` (the climate, the intentional mask and the routing rules the parameters do not name); `load_basin_table` reads `Ok(None)` for another spec and refuses a damaged record naming what it found.
- `generators::bake::lakes`: `LAKES_VERSION`, `TileRef`, `Lake::tiles` and `lakes_with_tiles` (the tiles of each lake water from a tile map), `Water::tiles`.
- STRM-080 added (streaming-cache); TEST-108 continues.
- Not here: the kind-13 version 3 and codec 3 for `L` (PR 2b), the write from the bake's tile writer and the `commit_id` read (A3 section 7.4) with it, river pieces (the rivers PR).

## Impact
- Code: `planet-cache` (`basin_table`, `key`), `generators::bake::lakes`; tests `cache/tests/basin_table` (4) and `generators/tests/bake_lakes` (the tile list). No stored bit of an existing kind changes.
- Numerics review: the encoder asserts the decoder's rules, the tile count is a `u32` (a lake of a fine grid holds more than 65,535 tiles), non-finite numbers and a dry lake with a level over its floor are refused on both sides, `inputs_id` joins the key, the producer asserts one tile list per basin.
