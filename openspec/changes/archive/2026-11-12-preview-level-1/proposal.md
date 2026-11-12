# preview-level-1 (briefing item 16, first step; inbox #95)

## What
Preview level 1 of A3 11.1 for the edit kinds height and landform stamp, and rainfall and temperature region: an H0 re-route over the edit, planet-wide with the heights resident, and the trunk rivers and watersheds as an overlay.

| Step | Requirements | Content | Load |
|---|---|---|---|
| 1 (this change's first PR) | AUTH-007 to AUTH-010, GEN-157, TEST-098 | `bake::preview`: the resident base, the edit, the re-route, the overlay data, the field id, the latest-wins slot; tier A on small grids | `test fast` only |
| 2 | TEST-099 | the latency example and scene: the seconds-scale number per edit kind at the H0 grid | after the explorer's re-run has ended |
| 3 | VIEW-067 | the overlay in the viewer and the editor | after step 2 |

## Not in this change
- The coarse-carve rows (lithology, class paint), level 2's display, item 15.
- `h0.rs`: the re-route calls `GlobalGrid::route` as it is.
- Item kinds for rainfall and temperature regions (none exists in `world-def`): the preview takes a `ClimateRegion` of its own; the editor path adds the item kind later.
