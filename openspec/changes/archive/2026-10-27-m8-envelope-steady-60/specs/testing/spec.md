## MODIFIED Requirements

### Requirement: TEST-057 The envelope scene flies a steady curve beside the cold one
With a chunk stage the envelope scene (TEST-047) SHALL search each altitude twice: the cold trial of TEST-054 (the first visit after a stop) and a steady-flight trial that flies a run-in at the speed under test before its measured stretch, with the chunk stage and the prefetch running. The stretch SHALL be `--steady-window-s` long (60 s by default) over ground the run-in did not cover and cross several chunk edges, and the bound SHALL be judged over every 1 s window of the whole stretch. The log SHALL give both curves, the run-in, the stretch and, per trial, the earliest time a chunk that landed was first wanted.

Verify: A, C
Status: active
Source: opus-planner (2026-10-27, m8-chunk-lead: the cold trial measures the start-up), owner (2026-10-27, #790: the floor applies to steady flight), TEST-047, TEST-054, STRM-042

Detail: The run-in is 12 s (a job latency of 5 to 7 s and 5 s more) and is not judged. A stretch of 12 s is the quick single-edge trial, with the chunk edge half way into it. The steady curve is not part of the smoke run beyond one short trial per altitude. The owner decides the floor applies to the steady curve.

#### Scenario: Steady and cold
- **WHEN** the scene runs with a chunk stage
- **THEN** the log holds the cold curve, the steady curve with its run-in and stretch, and the first-wanted time of each trial, and every steady trial crosses a chunk edge inside the measured stretch
