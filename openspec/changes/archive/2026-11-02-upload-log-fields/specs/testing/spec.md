## MODIFIED Requirements

### Requirement: TEST-031 Water plan counters in the streaming frames log
The `frames.txt` of a streaming case SHALL end with the columns `queries`, `tile_misses`, `band_queries`, `lit_plan_ms`, `deferred` and `node_bytes`, per frame: the water plan's field queries, book misses, band queries and milliseconds on the frame thread (all 0 on the debug path), the finished tiles that waited for the upload budget (the cap was reached) and the node-record bytes of the plan. With `upload_bytes`, `uploaded` and `upload_calls` they show whether a slow frame was an upload-capped one. They are recorded only: no verdict, count digest or comparison reads them. A log without the columns is refused as old-format.

Verify: A
Status: active
Source: M5 item 9a.1 (status.md), TEST-012, TEST-021

#### Scenario: The columns round-trip
- **WHEN** a frames log with counter values is written and read back
- **THEN** the values are equal and the header ends `queries tile_misses band_queries lit_plan_ms deferred node_bytes`

#### Scenario: A memoised plan asks nothing
- **WHEN** the same terrain plan is planned twice with one cache
- **THEN** the second plan's counter difference is 0, 0, 0
