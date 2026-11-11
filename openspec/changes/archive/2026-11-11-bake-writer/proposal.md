## Why
Item 15 PR 3 (architect, 2026-11-10, CP1): the bake has to write the new chain's L tiles, the basin tables and the lake water mask under one commit. Design: `openspec/changes/runtime-composition/design.md` section 14.

## What Changes
Part 1 (this PR, pure and light):
- `bake::water_mask` (GEN-167): the rule of design section 13 as a function of a window (core wet in the lake's water blocks under the level, a ring of at most 8 steps into cells that drain into the lake), the run-length encoding of the tile record's water section and its decoder, and the diagnostic of core-wet cells cut off from the lake's largest component.
- Tests M1-M6 (TEST-109) on an analytic surface: the valley below a sill and the valley sides, another hollow, an endorheic lake below its sill, partitions into tiles in any order, 1 and 8 threads, round trip and refusals.
Part 2 (architect, 2026-11-11: the three answers as recommended): the producer's `water` and `ring` lists (made in the pre-release step beside the pieces: about 20 MB at k=4, confirmed by the next k=4 run's MARK lines), `cache::l_writer` (`bake_l`: K7 over the tiles, the mask on `B + L` as stored, the six basin tables, a `ChannelFeed` trait that refuses areas, the commit record `layer-commit.txt`, `read_l_tile` / `read_basin_table` through it) with tests `cache/tests/l_writer` (STRM-082). The planet-scale feed is PR 7's graph; no planet-wide L bake before it.

## Impact
- Code: `generators::bake::water_mask` (new), `generators::bake::lakes` (water and ring lists, `river_pieces` returns `PreRelease`), `cache::l_writer` (new), tests `bake_water_mask`, `bake_lakes`, `l_writer`. No stored byte changes (the water section stays empty until the writer fills it); no key or golden moves.
- GEN-167, TEST-109, STRM-082 added.
