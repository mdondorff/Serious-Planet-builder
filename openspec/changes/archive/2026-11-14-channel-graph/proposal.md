## Why
Item 15 PR 7 (architect, 2026-11-10 and 2026-11-11): K7 over tiles needs the channels from the store, not from a hydrology held in memory, so that a planet-scale L bake is possible; the drawing (PR 8) reads the same stored `Q_w`. Design: `openspec/changes/runtime-composition/design.md` section 16.

## What Changes
- `bake::channel_graph` (GEN-170): the segments' cell chains as compact arrays (`ChannelGraph`: first cell, vertex count, a direction, a `Q_w` code and a bed code per vertex), built in the pre-release step; `Q_w` as a `u16` on a log scale (1e-4 to 1e7 m3/s, relative step 3.87e-4, a read within 1.94e-4), the bed level as an `i16` count of 0.5 m (the running minimum of the H1 surface); `channels_from_graph` (K7's input from the arrays); the bed code rounds down (never above the f64 level); per segment the vertex ranges under a lake (`lake_cut_ranges`, `PreRelease::cuts`) so that the drawing derives the producer's pieces from Kind 20; `Built::graph`.
- `Grid::cell_at_face` and `Cube::resolve` (GEN-170): the H1 cell one step outside a face resolves across the cube edge as the graph joins it: the face-seam resolver of PR 3's open limit (a corner quadrant has no cell).
- `cache::graph_store` (STRM-084): Kind 20, one record per L tile holding the pieces of the segments in the tile's block of H1 cells and one H1 cell beyond it; `cache::graph_feed::ChannelGraphFeed`: the feed over a commit; the commit records the graph blocks (commit layout version 3: `graph_cells` and `graph` lines).
- `l_writer::bake_l` takes an optional `GraphInput`; a graph in area units is refused.
- Tests (TEST-114): codes, chains, pieces round trip and refusals, the seam resolver, and the bake that reads its channels back through the commit giving the L tiles of the bake that had them in memory byte for byte.

## Not in this change
- The stored size of Kind 20 on the k=1 landmass (an example that writes the blocks of a plan's graph: next); channels of a neighbouring face in a face's plane (a segment across a face edge is two runs).

## Impact
- Code: `generators::bake::{channel_graph, cube, mod, build}`, `examples/landmass_scale` (prints the graph's bytes and error), `cache::{graph_store, graph_feed, l_writer, key}`, tests. Kind 20 and `PARAM_CHANNEL_GRAPH` 19 added. No existing key, record or golden moves (the commit layout is version 3: no commit of version 2 is read).
- Requirements GEN-170, STRM-084, TEST-114 added.
