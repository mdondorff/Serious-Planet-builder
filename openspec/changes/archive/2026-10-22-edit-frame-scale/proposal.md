# Change: edit-frame-scale

## Why
- SPIKE-29 fix 3 (opus-planner, 2026-10-22): `EditRuntime::update` took 5.6 s at 5,000 edits and 100 s at 20,000. A timer at 5,000: the outlines (one height query per shown edit, about 1.3 ms each in a dense cluster) were 97 % of it, and "shown" was every edit over the plan's wanted tiles (nearly all of the cluster at every altitude), because the relevance test has no screen test of the place.

## What changes
- `relevance::in_view` and `shown_items`/`shown_from`: the edits that can be seen, at most `MAX_SHOWN`, the largest on the screen first; the footprint find reads the direction and reach it already returns (no lookup per item).
- `Session::shown` keeps the footprint find per definition revision and tiles; `EditRuntime` keeps the outlines' ground heights per field item revision.
- The edit-scale scene measures the plan right after one edit.
- VIEW-051.

## Tests
- Tier A (`crates/editor/tests/shown.rs`): the cap and the order, the view, the reuse.

Expected golden changes: none.
