## Why
SPIKE-88's follow-up (inbox #61, #62): `gpu_edit_perf` failed once on WARP with the edit stale for 40 s, 8 nodes substituted, nothing pending and no job running. Hypothesis 1 of the analysis holds: an edit re-keys resident coarse tiles that the edit's find (`planet_editor::feed::edited_tiles`) does not name, so they are never marked stale and never requested again.
- A coarse tile (levels 0-5 at Earth's radius) reads its region summary once an item touches it (WDEF-008; a tile with an empty footprint keeps the base's key). Its cover reaches past its cap by `margin_rings`, sized for the 40 km maximum reach plus whole cells (about 13 rings, 450-500 km at level 5), and is the whole body for levels 0-2. Any edit anchored in the cover re-keys the tile; the find named it only when the edit's own reach touched the tile.
- Reproduced at drawn levels: an item in a level-5 tile and a 3 km-reach hill 250 km outside it re-key tiles of levels 3, 4 and 5 that the find did not name. This fits the flake and probably the Tracked "Drag residuals" (the scene and the editor put items near each other).

## What Changes
- **Fix (exact, option 2 of the planner, 2026-11-01):** `feed::edited_tiles_reading` also names every coarse resident tile that reads its summary and whose cover holds an affected anchor cell: one `neighbors8` walk from each anchor cell out to 64 rings (the walk of `cells::with_margin`) gives each coarse tile's fewest steps, compared with its `margin_rings` (the whole body when they overflow). No cover is built on any thread. The predicate `editwin::reads_summary` (a root, or an item of a stamp layer touches the tile with the stage's halo) comes from the editor's definition; `edited_tiles` without it is a conservative superset. The editor and the edit-latency scene use the exact form.
- **Tests (tier A):** `edit_keys.rs`: a key diff over every tile of levels 0-5 and the tiles round the edit for four edits; the counterexample above (14 named, 14 re-keyed); the find's halo equals the stamp stage's.
- **Diagnostics:** `Streamer::tile_report`, `queued_by_class`; the edit-latency error names the held refreshes, the queued requests per class and, per substituted node, the wanted against the resident key, membership in the edit's set and the streamer's state; the window's stall report names held and queued refreshes.
- Not changed: the width of the summary margin (ADR 0045 point 7 meant about one coarse tile per level per edit; today's margins re-key more): a Tracked architect item.

## Capabilities
### Modified Capabilities
- `streaming-cache`: STRM-070 added; STRM-026 (scenario)
- `testing`: TEST-041 (scenario)
