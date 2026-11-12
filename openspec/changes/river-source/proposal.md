## Why
Item 15 PR 8 (architect, 2026-11-12): the frame's river plan (REND-059) needs the channels of the commit the layer is read under, and the flight scene's cache peak needs a path that reads them. The viewer's stored world opens the new chain's commit for `--erosion`; it can hand the frame a river source over the same commit.

## What Changes
- `cache::river_read::RiverReader::commit` and `grid_dims` (the face edge and the L tile edge in cells and the nominal cell size, the K7 grid's `dx`).
- `planet::rivers::ReaderRiverSource` (STRM-086): a `RiverSource` over the reader; `WorldTerrain::rivers()` holds one when the commit the layer is read under holds a channel graph, and the erosion note says whether it does.
- `planet::rivers::plan_frame_rivers` and the viewer's per-frame call in `window.rs` (REND-062).
- Tier A flight over a baked one-face graph world (TEST-123): the reader's cache peak against its bound.
- Tests (tier A): the reader's grid dims equal the K7 grid's; a commit with no graph gives no river source and the note says so.

## Impact
- Code: `cache::river_read`, `planet::{rivers, worldfield, lib}`, tests. No key, record or golden moves.
- Requirements STRM-086, REND-062 and TEST-123 added.
