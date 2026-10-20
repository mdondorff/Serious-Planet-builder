## MODIFIED Requirements

### Requirement: TEST-041 Edit-latency stages
The `edit-latency` scene (TEST-019) SHALL record, for every run, where the time of the edit went, and print and write a second table after its budget table: the durations of the three calls of the edit in the editor's order (`WorldTerrain::set_definition`, `ManagedLoop::edit` with the find of the tiles, `set_preview` with the `preview_delta` that makes its argument); and, as instants in milliseconds from the call of `set_definition`, taken at the end of the frame that first shows the stage, the first frame after the edit that delivered a plan (with the worker's own time for that plan), that delivered the residency set R, in which a tile job had been handed to the spawner since the edit, in which half of the refresh requests and in which all of them had completed (the streamer's refreshed counter), and in which nothing waited for an upload and no job was in flight; and the median wall time of the frames of the run. The scene SHALL make the find of the tiles as the editor does (`planet_editor::feed::edited_tiles` over the resident tiles) and the table SHALL add, after the frame median, the duration of that find (inside the edit call) and the instant of the first frame after the edit whose draw plan is exact (`DrawKind::Exact`, every drawn tile has the plan's key), which may come before the idle frame the exact time waits for. The columns SHALL be the medians over the repeats of a case, `-` for a stage that did not happen, and the budget table SHALL read "exact" as the tiles the plan draws (ADR 0072): its exact column and its verdict are the median instant of the first frame after the edit whose draw plan is exact, `-` when it did not happen, and the time until nothing is left to do (every resident tile refreshed, no job in flight, no upload waiting) stays a column of the stages table, recorded and not gated. The scene takes `--settle-ms N` (the refresh settle window of ADR 0072, default 25 as the editor sets it; 0 holds nothing) and the stages table SHALL print it, with the three pre-emption counts of the run (refreshes coalesced, running jobs cancelled, results dropped unuploaded, STRM-031), so a run at 0 and a run at the default show the settle's share of a case. A third table SHALL give, for the first run of each case, the refreshed set by level: the level, its worst-case sample spacing (`LodParams::sample_spacing`), the tiles the edit named, how many of them the plan drawn before the edit contained (the others are resident only) and how many lie at a level whose sample spacing is above the stamp's largest change. The scene SHALL create the `--out` directory when it is missing.

Verify: C
Status: active
Source: window 13 (2026-10-19), window 14 (2026-10-19), window 15 (2026-10-20), TEST-019, M6 plan rows E1 and E3, ADR 0072

#### Scenario: The stages table
- **WHEN** the scene runs in smoke mode
- **THEN** after the budget table it prints the stages table with the columns above and a row for the case, the durations, the frame median and the find call are numbers, and `edit-latency.md` holds the tables

#### Scenario: The verdict reads the drawn tiles
- **WHEN** a case's runs have a first all-drawn-exact frame earlier than the idle frame
- **THEN** the budget table's exact figure and verdict use the drawn-exact median, the all-exact time is a column of the stages table, and the stages table prints the settle window and the three pre-emption counts

#### Scenario: The refreshed set by level
- **WHEN** the scene runs in smoke mode
- **THEN** it prints the refreshed-set table with at least one row for the case, the named tiles are the sum of the drawn and the resident-only ones, and `edit-latency.md` holds the table

### Requirement: TEST-042 The brush cases of the edit-latency scene
The `edit-latency` scene (TEST-019, TEST-041) SHALL run, after the hill cases of each altitude it is measured at 200 m and 2 km, brush cases: one brush item in the layer `brush` holding one raise stroke of 20 points a quarter of the radius apart, running east and centred on the place the views look at, with a strength of a fifth of the radius up to 100 m, a reach that holds the stroke and its radius and a bound on the change equal to the strength, at the radii 150 m and 1 km (`editperf::case_brush`). The case SHALL be edited and taken back as a hill case is (the same sequence and stages; the footprint find from the item's anchor and reach; the preview delta with the brushes (`preview_delta_on`, as the editor sets it), so the preview column reads the first frame that shows the chain of preview stamps), and its rows in the budget table, the stages table and the refreshed-set table SHALL print the radius as `<r> m brush` so that no brush row is read as a stamp. A smoke run SHALL add the 150 m case at 200 m, and fail when the brush edit never becomes exact. The brush rows are read against the same exact budgets by radius and altitude as the hill rows. A gated row with no frame that showed the preview SHALL be judged on its exact time alone and say `(no preview)` in its verdict; `no result` stays for a row with no exact frame.

Verify: C
Status: active
Source: opus-planner (2026-10-20), TEST-019, TEST-041, VIEW-035, docs/status.md (coarse-first refinement parked)

#### Scenario: The smoke run
- **WHEN** the scene runs in smoke mode on the software adapter
- **THEN** it prints a `200 m | 150 m brush` row in each of the three tables, the budget row has an exact time, the stages row has its durations, the settle window and the three pre-emption counts as numbers, and `edit-latency.md` holds the rows
