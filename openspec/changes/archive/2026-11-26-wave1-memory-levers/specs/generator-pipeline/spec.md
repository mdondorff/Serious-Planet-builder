## ADDED Requirements
### Requirement: GEN-192 Wave 1 runs on lean memory paths with the same bits
The region bake's wave 1 SHALL use the memory levers of the Detail's first table, each giving the bits of the path it replaces at any thread count (CON-14).

Verify: A
Status: active
Source: Lead 2026-11-26 (the RG park plan, step 3; the owner parked RG, 2026-11-26), GEN-143, GEN-147, CON-14
Detail:
| Lever | Where | What it frees |
|---|---|---|
| K5's in-flight cap | `RoundCache::with_inflight_cap` | the largest-first catchment queue runs in chunks of at most the cap's cells (a larger catchment alone), each chunk written back before the next gathers; a cache that keeps nothing drops a chunk's heights once written |
| the route's filled surface | `partitioned_route_weighted` | a second flood: the front takes the partitioned route's filled surface (GEN-143) |
| the weights | the region front | freed after the route |
| the initiation bits | `initiation_bits`, `hydrology_initiated` | the MFD area: hydrology's channel initiation is kept as one bit a cell |
| the cascade's order and flags | `hydrology` | freed before the basins and segments |

| B's region bake | Value |
|---|---|
| K5 | `solve_rounds_memo`, tile `REGION_TILE` (256), cap `INFLIGHT_SHARE` (0.06) of the cells that are not outlets, a cache that keeps nothing |
| the front | the partitioned route with the weights, its filled surface, the initiation bits, the area freed, `hydrology_initiated` |

Evidence (non-normative):

| Measured | Stages | Cells with the ring | Peak working set | Per cell |
|---|---|---|---|---|
| B, `landmass_scale 1 16 bwave1` (2026-11-26) | K5, K6 | 9.409e6 | 677 MiB (peak commit 690 MiB) | 74.9 B above a fixed 5 MiB |
| RG's wave 1 on the spike, k = 4 (tag `archive/rg-parked-2026-11-26`, commit d9c320dd7) | base1, K5, K6 | 1.4934e8 (1.489e8 land) | 8,834 MiB = 9.26 GB | 61.0 B a cell plus 136 MiB (the k = 1, 2, 4 fit) |
- B's figure is a measurement at k = 1; it is not extrapolated to the landmass.
- Not on main: base1's parallel decide (base1 is an RG stage) and RG's K6 levers of the budget mode; both stay on the tag.

#### Scenario: K5 capped
- **WHEN** K5 runs 3 rounds on a compact landmass of the cube and on a plane, with and without runoff drivers, capped at 1 and 7 cells, a sixteenth and a third of the land, with a cache that keeps nothing and with the memo
- **THEN** the surface equals K5 uncapped on one thread bit for bit, at 1 and 8 threads; a second solve on the memoising capped cache hits the memo and gives the same bits

#### Scenario: Hydrology on the initiation bits
- **WHEN** hydrology runs on the compact landmass and the plane, with and without open-water evaporation, once on the MFD area and once on its initiation bits with the area freed
- **THEN** q, q_w, the channel cells, the basin labels, the segments, the basins and the counts are the same

#### Scenario: The route's filled surface across tiles
- **WHEN** a window of 72 x 60 cells of a cube face of 2,048 cells (an origin and the face's metric) is routed with runoff weights, resident and in tiles of 8, 13 and 25 cells
- **THEN** the partitioned route's filled surface is the canonical flood's, and its receivers, order and weighted area are the resident route's, bit for bit

#### Scenario: B's region front
- **WHEN** the region front runs on faces of 64 and 2,048 cells a face edge with a climate that varies over the region, at 1 and 3 threads
- **THEN** the solved surface (against the resident rounds on one thread), the hydrology's counts, the lakes, the channel graph and the segment ids equal the resident pipeline's (`solve_rounds`, `route_rules`, `priority_flood_with`, `hydrology`)
