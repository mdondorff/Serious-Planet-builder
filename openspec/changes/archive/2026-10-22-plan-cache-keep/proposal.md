# Change: plan-cache-keep

## Why
- SPIKE-29: the plan right after one edit costs 70 to 75 % of a cold plan (3.7 s over a 5,000-item cluster, 2.4 s over empty ground), because the cache re-checks every tile's footprint hash and recomputes every certificate and cap bound whenever the item revision moves. opus-planner (2026-10-22) chose to invalidate only what the edit's places reach, with the global check kept for anything not announced. A timer showed the post-edit plan is the tree walk of the selection (`select`), its time the footprint hashes of thousands of tiles (a coarse tile's cover has about 20,000 nodes) and the cap bounds.

## What changes
- `WorldDefinition::item_changes_from` (WDEF-016); `GenContext::read_footprint` and a memo of the empty-subtree hash (WDEF-015: the same reads, without the item list a coarse tile does not record).
- `SelectionCache::note_item_change`, `ItemChange`, and the keep in `prepare` (STRM-034): the hashes of unreachable tiles stamped as checked, certificates and cap bounds outside eight radii kept (the certificates also while the levels that read bounds are the same); `invalidate_tiles` no longer clears them when a change was announced.
- `PlanWorker` and `ManagedLoop::note_item_change`; the window announces each edit it hands to the generator.
- The edit-scale scene measures the plan after an edit made in memory, announced or not.

## Tests
- Tier A: the oracle (a persistent cache equals a fresh one node for node after random edits, announced, chained, unannounced); the reach test against the real tile hashes; the diff of two definitions; the reads without the result.

Expected golden changes: none.
