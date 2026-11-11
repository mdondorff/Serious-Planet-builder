## Why
Item 15 PR 8 (the river drawing) reads the stored channel graph at run time; the writer's feed stops the process on a missing block, which a viewer must not do. Design: `openspec/changes/runtime-composition/design.md` section 18 (the read part, which does not depend on the drawing form).

## What Changes
- `cache::graph_feed::ChannelGraphFeed::try_channels_near` (STRM-085): the feed's read as a fallible call naming the block and commit; the writer's `channels_near` calls it and still stops.
- `cache::river_read::RiverReader` (STRM-085): the channels that can touch an L tile (its core widened by `DISC_RADIUS_MAX`), a bounded least-recently-used cache, every failure naming the tile; `CommitLayer::rivers` builds one under the layer's own commit.
- Tests (tier A, `crates/cache/tests/l_graph.rs`): the reader returns the feed's channels, the commit id equals the layer's, the cache holds its bound and an evicted tile decodes again to the same channels, a commit without a graph has no reader, a missing block is an error naming the tile.
- Two stale decision numbers in headers of `graph_feed.rs` and `l_layer.rs` replaced by the date (architect, 2026-11-11 rule).

## Not in this change
- The river plan and the drawing (REND-059, REND-060), G-PROFILE (TEST-116) and the draw scene (TEST-117): they wait for the architect's answers on design section 18 and for a G-PROFILE reading on the fixture that is not yet understood (reported separately).

## Impact
- Code: `cache::{graph_feed, river_read, l_layer, lib}`, tests. No key, record or golden moves.
- Requirement STRM-085 added.
