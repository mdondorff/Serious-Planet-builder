## MODIFIED Requirements

### Requirement: BUILD-054 repro replays a recorded path in a window and counts the stalls
`cargo xtask repro <frame-N-path.txt | bundle> --window [--real] [-- OPTIONS]` SHALL replay the camera path in real time in `planet editor --smoke` (the gate's path replay, TEST-061) over the world its bundle records (VIEW-071); options after `--` go to the editor. The run measures: the gate's failures are printed, not raised, and its output is kept under `target/repro-window`. It SHALL print the gate line (stalls of 1 s or more with the GPU phases counted, each with its frame's resident, written and evicted tiles; the held-back tiles and how many lie over the commit's region). With `--real` a locked desktop is NOT RUN. It SHALL refuse a non-terrain bundle, a missing path file, and `--at-commit`, `--every` or `--store`.

Verify: A
Status: active
Source: Lead 2026-11-13 (P2 E3 follow-up: the replay ended in the gate's held-back-tiles failure and never printed its closing lines), TEST-061, VIEW-071

#### Scenario: The stalls of a path
- **WHEN** the gate line of a replay reads "stalls 7 (frames of 1000 ms or more with the GPU phases, longest 6400 ms)"
- **THEN** the verdict says 7 stalls and the options of the world the run opened

#### Scenario: A stall with its counts
- **WHEN** a frame of 4.5 s has 23,100 tiles resident, 120 written and 4 evicted
- **THEN** the gate line names the frame, its longest phase and those three counts, so that a resident count is told from an upload burst

#### Scenario: A render submit counts
- **WHEN** a frame spends 4.5 s in its render submit and nothing else is slow
- **THEN** it is a stall of the report and not a failure of the gate's frame limit

#### Scenario: A replay that the gate would fail
- **WHEN** the plan holds tiles back with no edit in reach during a replay
- **THEN** the gate line says how many distinct tiles were held, at which levels and how many lie over the commit's region, the failure is printed as not judged, and the run ends normally with its closing lines
