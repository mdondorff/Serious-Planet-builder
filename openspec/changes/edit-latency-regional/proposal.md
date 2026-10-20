## Why
The M6 gate evidence list (docs/briefings/m6-gate-evidence.md, G2 and G3) found no test and no measurement for the regional scale (1 to 50 km), for a ridge and for a region edit (opus-planner, 2026-10-20). The scene measures what it has cases for.

## What Changes
- The edit-latency scene gets regional stamps (5 km and 40 km, from 2 km and 20 km altitude), one ridge of 5 km (30 m high, 200 m half-width, 17 points) and one region of 2 km whose surface type is changed in place (rock to forest), the last two from 200 m and 2 km (TEST-044). The largest regional stamp is 40 km, not 50 km: an item reaches one cell edge at most (`CELL_EDGE_M`, WDEF-007).
- The three kinds are labelled in the tables (`5000 m ridge`, `2000 m region`) and their verdict reads `recorded`: the budget is set with the owner after measuring, so the regional stamps (over 1 km), the ridge and the region are not gated.
- A case may have an earlier state that is in the world before the edit (the region); the scene puts it in, settles, runs the repeats and takes it out again.
- A smoke run adds the 5 km stamp, the ridge and the region at 200 m.
- Tier-A tests of the cases and the labels; the software-adapter smoke checks the three rows. No measurement: the next perf window reads them with the brush cases and the preview cost.

## Classes and bounds (ADR 0035)
No field, no shader value and no golden is touched.

## Out of scope
A budget for the regional scale (the owner, after measuring); a channel or a flatten case; the global scale.
