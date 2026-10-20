## ADDED Requirements

### Requirement: TEST-042 The brush cases of the edit-latency scene
The `edit-latency` scene (TEST-019, TEST-041) SHALL run, after the hill cases of each altitude it is measured at 200 m and 2 km, brush cases: one brush item in the layer `brush` holding one raise stroke of 20 points a quarter of the radius apart, running east and centred on the place the views look at, with a strength of a fifth of the radius up to 100 m, a reach that holds the stroke and its radius and a bound on the change equal to the strength, at the radii 150 m and 1 km (`editperf::case_brush`). The case SHALL be edited and taken back as a hill case is (the same sequence and stages; the footprint find from the item's anchor and reach; the preview delta over the items of the item's layer, which has no brush preview, so the preview column reads `-`), and its rows in the budget table, the stages table and the refreshed-set table SHALL print the radius as `<r> m brush` so that no brush row is read as a stamp. A smoke run SHALL add the 150 m case at 200 m, and fail when the brush edit never becomes exact. The brush rows are read against the same exact budgets by radius and altitude as the hill rows.

Verify: C
Status: active
Source: opus-planner (2026-10-20), TEST-019, TEST-041, VIEW-035, docs/status.md (coarse-first refinement parked)

#### Scenario: The smoke run
- **WHEN** the scene runs in smoke mode on the software adapter
- **THEN** it prints a `200 m | 150 m brush` row in each of the three tables, the budget row has an exact time, the stages row has its durations, the settle window and the three pre-emption counts as numbers, and `edit-latency.md` holds the rows
