## Why
Window 9 and the phase clocks of 2026-10-27 (`docs/milestones/M8/criterion-6-edit-window.md`): one chunk re-solve takes 4.2 to 6.5 s; the network phase is 2.0 to 3.1 s of it and the base heights are 1.7 to 2.5 s of that (about 80 % of the network, 40 to 45 % of the job). Every edit reads all the heights again, also those far from the edit. Planner (2026-10-27): reuse them, exact (same bits as a full re-solve), with a test.

## What Changes
1. `BaseDetail::region_heights(dirs)` (default `base_heights`): the heights of one chunk region. `WorldBase` keeps each batch with the definition it was read under (`RegionCache`, at most 24 batches and 256 MiB, least recently used out) and, for a later definition that differs only in its items (`WorldDefinition::same_but_items`), reads again only the cells within the reach of the items that differ (`item_changes_from`: old and new place of every changed, added, removed or moved item). A change that reaches more than half the batch reads all of it.
2. The batch is kept only if the definition did not move while it was read and every answer was real.
3. (2026-10-27, brush) For a brush the cells to read again are the ones within the radius of a stroke (`WorldDefinition::changed_items_from`, `footprint::strokes_touch`), not the reach disc that holds the whole stroke (several times its area): a 1 km brush at the edit-latency site reads 12 % of the cells again instead of 24 to 88 %.
4. `generate --edit-resolve` (`--brush` puts the brush case) reports the share of region cells reused and the time of the base heights per chunk job.

## Design notes
- Exactness rests on WDEF-010 / GEN-026 ("an item changes no output of any stage beyond its `reach_m`") and on point evaluation: the height at a cell is a function of that cell and the items, whatever the batch or the thread count (CON-14). The reach test is the footprint module's one answer (`EditReach::within_angle`, widened; a cell exactly at the edge counts). Line items and brushes take the whole disc.
- Memo bound: 24 batches; a region has about 0.4 to 0.8 million cells (8 bytes each), so 3 to 6 MiB per batch and at most about 150 MiB, 256 MiB as the hard cap.
- Not in this change: the router's and the anchors' height reads (`RouteMemo`, emptied at every epoch) and the relief factors; measured after this change, they are the next cut if the network phase stays large.

## Impact
- Spec: GEN-087 (new). Code: `world-def/lib.rs` (`same_but_items`), `generators/anet.rs` (`region_heights`, the clock), `app/regionheights.rs` (new), `app/anetworld.rs`, `app/editresolve.rs`. Tests: unit in `regionheights.rs`, `app/tests/oracle_edit_resolve.rs` (tier B, bits equal a cold solve).
