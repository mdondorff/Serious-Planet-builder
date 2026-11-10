## Why
Item 15 PR 3 (architect D17/D18/D20, CP1): the bake has to write the new chain's L tiles, the basin tables and the lake water mask under one commit. Design: `openspec/changes/runtime-composition/design.md` section 14.

## What Changes
Part 1 (this PR, pure and light):
- `bake::water_mask` (GEN-167): the rule of design section 13 as a function of a window (core wet in the lake's water blocks under the level, a ring of at most 8 steps into cells that drain into the lake), the run-length encoding of the tile record's water section and its decoder, and the diagnostic of core-wet cells cut off from the lake's largest component.
- Tests M1-M6 (TEST-109) on an analytic surface: the valley below a sill and the valley sides, another hollow, an endorheic lake below its sill, partitions into tiles in any order, 1 and 8 threads, round trip and refusals.
Part 2 (after the architect's answers on the channel feed, the commit record and the producer lists): the producer's `water` and `ring` lists, the writer, the commit record (STRM-082).

## Impact
- Code: `generators::bake::water_mask` (new), tests `bake_water_mask`. No stored byte changes (the water section stays empty until the writer fills it); no key or golden moves.
- GEN-167 and TEST-109 added; STRM-082 follows with the writer.
