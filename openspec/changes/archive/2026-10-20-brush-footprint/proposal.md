## Why
Window 19 (`docs/perf/window-19.md`): a 150 m brush stroke at 200 m altitude re-keys 1,816 resident tiles against 291 for a 150 m stamp.
- A brush item's reach is a disc round the stroke's first point that holds the whole stroke: radius about 864 m for the 712 m stroke, 2.35 km² against the stroke's own 0.28 km².
- The footprint query returned the item for every tile of that disc (levels 15 to 19: 3.3 to 7.3 times the tiles the stroke reaches).
- A stroke changes no output beyond its radius from its polyline (GEN-030), so the extra tiles are re-keyed, regenerated and re-uploaded for nothing.

## What changes
- `world_def::footprint::strokes_touch`: a brush item with strokes is returned only when some stroke's polyline is within its radius plus the cap's angle of the cap's centre, after the reach test. `arc_angle`, `polyline_angle`, `stroke_angle` and `point_of` move from `generators::stamp` to `world_def::footprint` (the same code; `generators::stamp::polyline_angle` is re-exported).
- `planet_editor::commands::Affected` carries the brush stamp of the item state (`brush: Option<Arc<Stamp>>`, no longer `Copy`); `feed::edited_tiles` names a resident tile for a brush only when a stroke touches it. This is the set `ManagedLoop::edit` refreshes (the 1,816 tiles of window 19); the footprint query alone changes only keys.
- An edit that appends a stroke to a large item still names the capsules of all the strokes of the item before and after (false positives only cost a refresh).
- The editor's view relevance (VIEW-015) takes the footprint find too: a brush is shown when a stroke reaches a visible tile, not whenever its reach disc does.
- `editperf` (harness): the brush rows feed the preview with `preview_delta_on` as the editor does (they read `-` because the harness called `preview_delta`, which has no brushes); a gated row without a preview frame is judged on its exact time (`ok (no preview)` / `OVER (no preview)`); the `--out` directory is created.
- Specs: WDEF-007 (the brush stroke test, one scenario), VIEW-015 and VIEW-019 (the stroke names the tiles), TEST-041 (the `--out` directory), TEST-042 (the brush preview and the verdict).
- Tile keys of tiles that held a brush item only through its disc change once (the item leaves their read set); no tile of the stroke's reach changes.

Expected golden changes: none.
