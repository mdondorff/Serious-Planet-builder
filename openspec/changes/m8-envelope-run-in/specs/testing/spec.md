## ADDED Requirements

### Requirement: TEST-057 The envelope scene flies a steady curve beside the cold one
With a chunk stage the envelope scene (TEST-047) SHALL search each altitude twice: the cold trial of TEST-054 (the first visit after a stop) and a steady-flight trial that flies a run-in at the speed under test before its measured window, with the chunk stage and the prefetch running and the chunk edge half way into the window. The log SHALL give both curves, the length of the run-in and, per trial, the earliest time a chunk that landed was first wanted.

Verify: A, C
Status: active
Source: opus-planner (2026-10-27, m8-chunk-lead: the cold trial measures the start-up), TEST-047, TEST-054, STRM-042

Detail: The run-in is 12 s (a job latency of 5 to 7 s and 5 s more) and is not judged. The steady curve is not part of the smoke run beyond one short trial per altitude. Which curve the floor of criterion 7 applies to is the owner's decision.

#### Scenario: Steady and cold
- **WHEN** the scene runs with a chunk stage
- **THEN** the log holds the cold curve, the steady curve with its run-in, and the first-wanted time of each trial, and every steady trial has its chunk edge inside the measured window
