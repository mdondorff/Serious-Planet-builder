## MODIFIED Requirements

### Requirement: TEST-060 Milestone M8 acceptance
`cargo xtask accept M8` SHALL run the automated steps of `accept M5`, the discrete-GPU tier C run and `gate --small` (TEST-063), and SHALL judge M8's criteria 4 to 7 only from the logs of a valid window, never from a measurement of its own. A missing, smoke, other-scene or overridden log SHALL be MISSING, not a pass; a failed row fails the command, otherwise a MISSING row makes it incomplete. Criterion 3 (`cargo xtask gate` on the real store, chunk stage off) SHALL be listed as a manual item.

Verify: A
Status: active
Source: opus-planner (2026-10-28), TEST-026, GEN-042, REND-044, TEST-042, TEST-047, TEST-061 to TEST-063, owner fallback 2026-10-28, owner decision 2026-10-29 (criterion 6)

Detail: The logs in `perf/logs/` or `--logs DIR`: `coupled-make.json` (at most 600 s from the 16384 bake), `shadow-cost.json` (every pose at most 1.5 ms by the compiled-out draws), `edit-latency.md` (criterion 6, amended by the owner 2026-10-29: no row over in the M6 table headed `verdict`, the drawn tiles within budget; the horizon tables and the chunk table `verdict (10 s, drawn tiles)` are not judged) and `envelope.json` (steady speed at 300 m at least 200 m/s; its `chunk_stage` SHALL be false, its `horizon` true and its `source` the production store, not an in-memory bake: otherwise, or when a field is absent, the row is MISSING).

#### Scenario: Logs against the budgets
- **WHEN** the four logs are within their budgets, then one is over, then they are missing, smoke or of another scene
- **THEN** the rows pass, the over row fails and names its number, and the others are MISSING; the outcome is pass, fail and incomplete, and a failure wins over a missing log

#### Scenario: The edit log's tables
- **WHEN** the chunk table has an over row, the horizon table is absent, then present with an over row
- **THEN** criterion 6 passes in all three cases while the M6 table is within budget, and the horizon rows are not judged; an over row of the M6 table FAILS and a log without the M6 table is MISSING

### Requirement: TEST-065 The gate reports the horizon re-bake after its edit; the edit-latency scene tabulates it
The gate SHALL fail a lit run with the horizon jobs on when no horizon map was baked. It SHALL report, and not judge, the time from the scripted edit to the first landing of a map the edit touched, or that none landed (owner decision 2026-10-29: the horizon-map shadows are redone after SPIKE-56, M8 criterion 6 is decided by the drawn tiles). The edit-latency scene SHALL print a table headed `edit to map ms`, one row per touched map, with no verdict column.

Verify: A
Status: active
Source: opus-planner M8-F step 2, TEST-061, owner decision 2026-10-29

Detail: A map is on screen with its landing plus the frames of its upload; the window puts one landed map on the GPU per frame (`HorizonRig::decide`), so within one frame of landing when it is the only one pending. The window gate logs the time and frame at which the first re-baked map is drawn after its edit. A touched map is one whose edit word changed at the edit. Window 28 for reference: 131 of 269 maps over 10 s while every drawn-tile case was within budget.

#### Scenario: A slow re-bake
- **WHEN** the first map landing after the edit comes 11 s after it, or none comes
- **THEN** the gate passes and its report line names the time or that none landed

#### Scenario: No map baked
- **WHEN** a lit run with the horizon jobs on baked no horizon map
- **THEN** the gate fails naming the horizon maps

### Requirement: TEST-066 The edit-latency horizon rows are measured on a quiet horizon
The edit-latency scene, with the horizon jobs and no chunk stage, SHALL:
1. make each edit of the table `edit to map ms` only after the horizon jobs have had no bake in flight and no start for at least 1.5 s;
2. wait for the same quiet after taking the edit back;
3. run the first case of each altitude once more, right after the view settled, into a table without a verdict column.

Verify: A
Status: active
Source: opus-planner (2026-10-29), TEST-065, window 27 per-bake log, owner decision 2026-10-29

Detail: A bake in flight when an edit lands is dropped and started again (STRM-060). The quiet window is longer than the jobs' settle window (`HorizonConfig::default().settle_ms`, 1 s); a unit test pins that. Neither horizon table is judged (TEST-065); a budget for an edit during a wave needs an owner decision. The bake diagnosis block of an edit names the bakes already running when it landed and the seconds waited for quiet. `accept M8` reads only the M6 table (TEST-060). With a chunk stage the wait is not made.

#### Scenario: The wave rows
- **WHEN** the scene ran with the horizon jobs and an informational wave run exists
- **THEN** the wave table has a row per touched map with its edit-to-map time and no `verdict` column

#### Scenario: Quiet
- **WHEN** a bake is in flight or a start happens, then neither for 1.5 s
- **THEN** the wait goes on, then ends
