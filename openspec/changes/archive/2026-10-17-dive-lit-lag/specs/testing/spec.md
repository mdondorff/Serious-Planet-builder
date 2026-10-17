## ADDED Requirements

### Requirement: TEST-031 Water plan counters in the streaming frames log
The `frames.txt` of a streaming case SHALL end with the columns `queries`, `tile_misses`, `band_queries` and `lit_plan_ms`: the field queries, the tiles asked of the tile jobs' water book that had no entry, the coastal-band queries and the milliseconds of the lit path's water plan on the frame thread, per frame (all 0 on the debug path). They are recorded only: no verdict, count digest or comparison reads them. `WaterTileCache::counters()` gives the cumulative counts; a frame's share is the difference of two snapshots. A log without the columns is refused as old-format.

Verify: A
Status: active
Source: M5 item 9a.1 (status.md), TEST-012, TEST-021

#### Scenario: The columns round-trip
- **WHEN** a frames log with counter values is written and read back
- **THEN** the values are equal and the header ends `queries tile_misses band_queries lit_plan_ms`

#### Scenario: A memoised plan asks nothing
- **WHEN** the same terrain plan is planned twice with one cache
- **THEN** the second plan's counter difference is 0, 0, 0
