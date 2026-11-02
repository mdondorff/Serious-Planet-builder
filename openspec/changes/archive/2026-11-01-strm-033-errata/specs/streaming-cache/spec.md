## ADDED Requirements

### Requirement: STRM-069 Cover tile states
The frame that draws the cover (`DrawKind::Cover`, STRM-016) SHALL label each cover tile by whether it is the tile the plan of the frame's camera would draw: a resident tile drawn whole at the level the plan's selection gives it is exact; only a tile that stands in for a tile of the plan that is not resident (a coarser tile, a tile drawn over some quadrants, or a tile the balancing made by splitting one of those) is a stand-in. With every tile of the residency set resident no cover tile is a stand-in, so the F3 stale-tile overlay (VIEW-014) does not show a camera that only turns as a stand-in.

Verify: A
Status: active
Source: owner (2026-10-21, P1-4), STRM-016, VIEW-014; added by turn-cover (#522) as STRM-033, an ID already taken by the RAM share, so never synced; synced as STRM-069 (opus-planner, 2026-11-01)

#### Scenario: A turn
- **WHEN** the camera turns away from the view of the plan in hand over a residency set that is complete, and again over one with tiles missing
- **THEN** the first frame draws the cover with every tile exact; the second draws exact the tiles at the plan's level and stand-in exactly the tiles that stand in for a missing one

## MODIFIED Requirements
### Requirement: STRM-033 The RAM share of terrain is capped by counts
Terrain's RAM share SHALL be 3 GiB of the 8 GiB of CON-21. Its parts SHALL be capped by counts asserted at tier A (CON-20): the bake blocks alive at most 0.5 GiB (20 blocks of 1024 cells while baking; the runtime reader's cache holds at most 512 MiB, `SOURCE_CACHE_BYTES`, 31 blocks of 1024 cells), the tile pack index at most 1 GB, staged and in-flight tile bytes at most 0.1 GB, plan caches at most 0.1 GB, the chunk horizon's height tiles at most 64 (96.5 MB at 614 cells, STRM-059), and generated but deferred tiles at most 2,048. The process working set is measured in the dive perf scene (TEST-019), not in a test.

Verify: A
Status: planned
Source: ADR 0033 point 8 (accepted 2026-10-16), opus-planner (2026-10-22, M7 H2: a cap a test asserts becomes active; the rest stay planned), STRM-059; the reader's cache from 16 blocks to `SOURCE_CACHE_BYTES` (ADR 0033 point 6) and 0.5 GB to 0.5 GiB, errata (SPIKE-89, opus-planner 2026-11-01)

#### Scenario: A cap
- **WHEN** a part of the share reaches its cap
- **THEN** the count stops at the cap (blocks evicted, tiles deferred or refused) and the test names the part, its count and its cap
