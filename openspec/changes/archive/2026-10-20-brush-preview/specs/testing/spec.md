## ADDED Requirements

### Requirement: TEST-043 The preview cost of the edit-latency scene
The `edit-latency` scene (TEST-019, TEST-041, TEST-042) SHALL print, after its other tables and write to `edit-latency.md`, a preview cost table: for each altitude, on the last view of that altitude, the frame planned and drawn with 0, 8 and 32 (`MAX_PREVIEW_STAMPS`) active preview stamps (a row of hill stamps of reach 400 m, a third of a reach apart, set as the transient preview, so that every flagged node evaluates all of them), the number of flagged nodes against the nodes of the plan, the median GPU time of the frame from the timestamp queries when the device has them (`-` when not), the median wall time until the GPU finished it, and the number of timed renders (20; 3 in a smoke run). The numbers are for the perf window to read; a smoke run only checks that the table is printed with 0, 8 and 32 stamps, that the wall times are numbers, and that no stamp flags no node and 32 stamps flag some.

Verify: C
Status: active
Source: opus-planner (2026-10-20), REND-033, VIEW-036, TEST-041, TEST-042

#### Scenario: The smoke run
- **WHEN** the scene runs in smoke mode on the software adapter
- **THEN** it prints the preview cost table with the rows 0, 8 and 32 for the altitude, the wall times are numbers, 0 stamps flag 0 nodes and 32 stamps flag at least one
