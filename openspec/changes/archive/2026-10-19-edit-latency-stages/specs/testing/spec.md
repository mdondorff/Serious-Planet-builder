## ADDED Requirements

### Requirement: TEST-041 Edit-latency stages
The `edit-latency` scene (TEST-019) SHALL record, for every run, where the time of the edit went, and print and write a second table after its budget table: the durations of the three calls of the edit in the editor's order (`WorldTerrain::set_definition`, `ManagedLoop::edit` with the find of the tiles, `set_preview` with the `preview_delta` that makes its argument); and, as instants in milliseconds from the call of `set_definition`, taken at the end of the frame that first shows the stage, the first frame after the edit that delivered a plan (with the worker's own time for that plan), that delivered the residency set R, in which a tile job had been handed to the spawner since the edit, in which half of the refresh requests and in which all of them had completed (the streamer's refreshed counter), and in which nothing waited for an upload and no job was in flight; and the median wall time of the frames of the run. The columns SHALL be the medians over the repeats of a case, `-` for a stage that did not happen, and the budget table and its verdicts SHALL be unchanged.

Verify: C
Status: active
Source: window 13 (2026-10-19), TEST-019, M6 plan rows E1 and E3

#### Scenario: The stages table
- **WHEN** the scene runs in smoke mode
- **THEN** after the budget table it prints the stages table with the columns above and a row for the case, the durations and the frame median are numbers, and `edit-latency.md` holds both tables
