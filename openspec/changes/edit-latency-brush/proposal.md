## Why
The editor has a brush (VIEW-035) and the edit-latency scene (TEST-019, TEST-041) measures only a hill stamp. A brush stroke names more tiles than a stamp of its radius and shows no preview, so its time to exact tiles is the revisit trigger of the parked coarse-first refinement of an edit (opus-planner, 2026-10-20; docs/status.md).

## What Changes
- The scene gets brush cases: one raise stroke of 20 points a quarter of the radius apart, centred on the place the views look at, as one brush item in the layer `brush`, at the radii 150 m and 1 km and the altitudes 200 m and 2 km (`editperf::case_brush`, `BRUSH_RADII_M`, `BRUSH_ALTITUDES_M`). A smoke run adds the 150 m case at 200 m after the hill case on the same rig.
- The case is edited and taken back as the hill is (the same `run_once`: `set_definition`, the footprint find from the item's anchor and reach, `ManagedLoop::edit`, `set_preview` with the delta of the layer; the preview shows no brush, so its column reads `-`), and the three tables carry its rows, the radius column reading `150 m brush`.
- TEST-042; tier-A tests of the case and the table; the existing software-adapter smoke checks the brush row, its stages row (numbers and the pre-emption counts) and its refreshed-set rows. No measurement: the numbers come from the next perf window.

## Classes and bounds (ADR 0035)
No field and no shader value is added; no golden moves. The scene's budgets are unchanged; a brush row is read against the same exact budgets by radius and altitude (a 1 km brush has the large one).

## Out of scope
A measured run (`cargo xtask perf stream edit-latency`, a perf window); the coarse-first refinement itself; a brush preview.
