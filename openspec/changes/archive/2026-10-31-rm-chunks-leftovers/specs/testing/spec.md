## MODIFIED Requirements

### Requirement: TEST-054 The envelope scene runs the horizon jobs
The envelope scene (TEST-047) SHALL run the horizon jobs from the base terrain when its world has a store, (ADR 0086: no stage runs on top of the base). The jobs are driven every frame from the plan's tiles and every flight starts with no horizon map resident. A world without a store SHALL keep the tile-only scene.

Verify: A
Status: active
Source: M8 plan, TEST-047, STRM-064, ADR 0086

#### Scenario: The stage of a flight
- **WHEN** the envelope scene is set up with a store and with an in-memory bake
- **THEN** the first runs the horizon jobs from the base terrain, and the second has none

#### Scenario: A frame waits for the horizon jobs
- **WHEN** a frame has a horizon job in flight, and when it has none
- **THEN** the frame is busy in the first case and not in the second, and the settle before a flight waits for a frame that is not busy

### Requirement: TEST-060 Milestone M8 acceptance
`cargo xtask accept M8` SHALL run the automated steps of `accept M5`, the discrete-GPU tier C run and `gate --small` (TEST-063), and SHALL judge M8's criteria 4 to 7 only from the logs of a valid window, never from a measurement of its own. A missing, smoke, other-scene or overridden log SHALL be MISSING, not a pass; a failed row fails the command, otherwise a MISSING row makes it incomplete. Criterion 3 (`cargo xtask gate` on the real store) SHALL be listed as a manual item.

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

### Requirement: TEST-061 The no-degradation gate counters and limits
The window SHALL, when `PLANET_GATE` is set, count the main-thread time of every frame, the tiles the plan holds back (ADR 0074), the horizon maps baked, the bake blocks the main thread decodes itself (VIEW-061) and the panics caught, and the gate SHALL fail the run on a frame over 100 ms, a held-back tile with no edit in reach, a decoded block, no horizon map over a lit run with the horizon jobs, a caught panic, or a step between levels over the bound of M8 exit criterion 3. A failure SHALL name the counter, the frame, the phase or the tile IDs.

Verify: A, C
Status: active
Source: owner (2026-10-28), owner test 2026-10-27 findings 7, 11 and 16, ADR 0074, TEST-058, VIEW-061

Detail: A frame's time is its phases without the waits for the GPU, and the phase of the longest one is named. Held-back tiles are listed with their IDs and whether an edit is in reach; those under an edit are reported and not failed. The first 20 frames are not judged. On the software adapter (`PLANET_GATE_SOFTWARE`, the CI run) frame time, decoded blocks and horizon maps are reported and not judged, because its rendering shares the CPU with the main thread; held-back tiles and panics are always judged. The step between levels is measured by `cargo xtask gate` (the field-side `anet_steps_report`, levels above the entry level), not by the window.

#### Scenario: A held-back tile away from an edit
- **WHEN** the counters record a frame whose plan holds back 3 tiles and no edit reaches them
- **THEN** the verdict fails naming the 3 tile IDs, and the frame

#### Scenario: A held-back tile under an edit
- **WHEN** the counters record held-back tiles all within the reach of an edit
- **THEN** the verdict passes and the report lists the tiles as flagged

#### Scenario: A slow frame
- **WHEN** a frame takes 140 ms of main-thread time in the phase "chunk frame"
- **THEN** the verdict fails naming the frame, 140 ms and the phase

#### Scenario: A block decoded by the main thread
- **WHEN** the main thread decodes a bake block in the phase "plan and residency (begin_frame)" of frame 31
- **THEN** the verdict fails naming the count, frame 31 and the phase

### Requirement: TEST-064 The gate runs on the base terrain
`cargo xtask gate` SHALL run the sites of TEST-062 over the base terrain and SHALL measure the step between levels on the base field: the tiles under one river's tile at levels 12 to 16 are generated and each vertex they share with their parent is compared with the parent's height, p99 and maximum in metres, with the bound 0.25 m (`base_steps_report`). The row names the figures ("measured on the base field"). The stage-on run (`gate --chunks`) no longer exists (ADR 0086). `gate --small` SHALL run the small world once.

Verify: C
Status: active
Source: opus-planner (2026-10-28), owner fallback 2026-10-28, TEST-061, TEST-062, ADR 0086

#### Scenario: Base terrain
- **WHEN** the window runs over a store with a coupled run
- **THEN** no chunk job is computed and the gate line names the horizon jobs and no chunk jobs; the levels row says it was measured on the base field

### Requirement: TEST-066 The edit-latency horizon rows are measured on a quiet horizon
The edit-latency scene, with the horizon jobs, SHALL:
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

### Requirement: TEST-075 The definition hash and the production store survive the removal
A world definition SHALL hash to the value it had before ADR 0086's removal: without items, with an item and the default `Under` setting, and with the layer set `OnTop`. The `chunk_stage` setting SHALL stay parseable and hashed, so a store built before the removal opens without a rebuild. The production store (`C:/DEV/SeriousPlanetData/global16384`) SHALL open with the current build, read only: `cargo xtask gate` runs the ignored `the_real_store_opens_without_a_rebuild` and prints MATCH, NOT RUN (the store is absent; never a pass) or a failure.

Verify: A
Status: active
Source: opus-planner (2026-10-31), ADR 0086 decision 3, GEN-039, WDEF-018

Detail: The expected values were computed on main before the generator cut; the test pins them as literals. The store records no whole-definition hash: a record is found by the keys its definition derives, so the check opens the store as the viewer does (the packs load without a problem, the bake, ocean channel, wet floor and water summary open) and loads the stored coupled run by its key; it writes nothing.

#### Scenario: The pinned hashes
- **WHEN** the three definitions are built and hashed
- **THEN** each equals its pinned value and `CHAIN_ORDER_ERA` is 2

#### Scenario: The production store
- **WHEN** the default world (seed 1, Earth radius) is opened over the store of 16384 cells and its coupled run is loaded by key
- **THEN** both succeed with no rebuild, or the check says NOT RUN when the store is absent
