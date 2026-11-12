## Why
Item 15 PR 8 (architect, 2026-11-12): the frame's river plan (REND-059) needs the channels of the commit the layer is read under, and the flight scene's cache peak needs a path that reads them. The viewer's stored world opens the new chain's commit for `--erosion`; it can hand the frame a river source over the same commit.

## What Changes
- `cache::river_read::RiverReader::commit` and `grid_dims` (the face edge and the L tile edge in cells and the nominal cell size, the K7 grid's `dx`).
- `planet::rivers::ReaderRiverSource` (STRM-086): a `RiverSource` over the reader; `WorldTerrain::rivers()` holds one when the commit the layer is read under holds a channel graph, and the erosion note says whether it does.
- Tests (tier A): the reader's grid dims equal the K7 grid's; a commit with no graph gives no river source and the note says so.

## Not in this change
- The viewer's per-frame call of `plan_rivers` and the flight scene with the cache peak: they need `TerrainPlan.rivers` (PR #1397) and follow it on this branch.

## Impact
- Code: `cache::river_read`, `planet::{rivers, worldfield, lib}`, tests. No key, record or golden moves.
- Requirement STRM-086 added.
