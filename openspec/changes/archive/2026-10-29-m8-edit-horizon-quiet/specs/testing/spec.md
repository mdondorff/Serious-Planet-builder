## ADDED Requirements

### Requirement: TEST-066 The edit-latency horizon rows are measured on a quiet horizon
The edit-latency scene, with the horizon jobs and no chunk stage, SHALL:
1. make each edit of the table `verdict (10 s, horizon maps)` only after the horizon jobs have had no bake in flight and no start for at least 1.5 s;
2. wait for the same quiet after taking the edit back;
3. run the first case of each altitude once more, right after the view settled, into a table without a verdict column, which is never judged.

Verify: A
Status: active
Source: opus-planner (2026-10-29), TEST-065, window 27 per-bake log

Detail: A bake in flight when an edit lands is dropped and started again (STRM-060). The quiet window is longer than the jobs' settle window (`HorizonConfig::default().settle_ms`, 1 s); a unit test pins that. The judged rows still hold the 10 s verdict of TEST-065. The wave run is one sample and has no budget, so it is not judged; a budget for an edit during a wave needs an owner decision. The bake diagnosis block of an edit names the bakes already running when it landed and the seconds waited for quiet. `accept M8` reads only the table headed `verdict (10 s, horizon maps)`. With a chunk stage the wait is not made.

#### Scenario: The wave rows
- **WHEN** the scene ran with the horizon jobs and an informational wave run exists
- **THEN** the wave table has a row per touched map with its edit-to-map time and no `verdict` column

#### Scenario: Quiet
- **WHEN** a bake is in flight or a start happens, then neither for 1.5 s
- **THEN** the wait goes on, then ends
