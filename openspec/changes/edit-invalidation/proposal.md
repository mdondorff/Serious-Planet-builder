## Why
M6 exit criterion: an edit is visible within 1 s. SPIKE-13 (variant D) measured the floor: exact in 55 to 103 ms at 2 km altitude, 144 to 689 ms at 200 m, 751 to 831 ms at 1 m, against 0.33 to 1.5 s today, once three things hold: the affected tiles are found by footprint, a stale tile stays drawn until its new version is resident, and an edit forces a plan. ADR 0066 (accepted) decides the keys: per-item `reach_m`, footprint query kind 3, region summaries kind 4, chained stage keys, `max_delta_m` bounds. This change builds the invalidation half of the editor (plan row E1); the stamp stage (E2), the preview (E3) and the commands (E4) are other changes.

## What Changes
Three steps, each its own PR, in this order.
1. `edit-invalidation-reach` (world-def, cache, generators): the item carries `reach_m` and `max_delta_m`; a tile reads a layer through query kind 3 (bounding-sphere test of the item's reach against the tile and halo), result material under `planet-builder footprint result v1`, format tag 3; the cell index stays the lookup. WDEF-005 is modified: cells stay the index, the footprint is the invalidation unit. M3 gate condition 2 lands here: the per-tile node set id (STRM-010 / ADR 0025 point 3), the declared-versus-recorded reads contract test (ADR 0021 point 7) and a tile-level GEN-017 test.
2. `edit-invalidation-refresh` (streaming, frame): key-aware refresh in the streamer (a planned key different from the resident key is a request of the same tile), stale-while-revalidate (the old tile is drawn until the new one is uploaded), an edit event that forces a plan and invalidates the key memo and cached volumes of the affected tiles, and a request class for edited tiles above the rest of R (STRM-005).
3. `edit-invalidation-bounds` (frame, terrain-lod): the `SelectionCache` key gains the 128-bit result hash of the tile's query; a tile's height bounds widen by the sum of `max_delta_m` of its items; the steep-field check at levels 16 to 18 (no empty morph interval).

## Impact
- Targets (perf window, indicative until then): edit-to-exact at most 300 ms up to 150 m stamps, at most 1 s up to 500 m stamps or at 1 m altitude.
- Goldens: none expected (no authored stage exists yet); step 1's design states whether the format tag 3 re-keys the tiles of worlds without items (to be checked against the key code before step 1 starts).
- Cache format: query kinds 3 and 4 are new entry kinds (ADR 0018, ADR 0023); the migration of existing stores is decided in step 1's design.
- Out of scope: the stamp stage (E2), the vertex-stage preview (E3), commands and item files (E4), global-scale authoring, the blob store.
