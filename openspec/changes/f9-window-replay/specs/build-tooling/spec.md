## ADDED Requirements
### Requirement: BUILD-054 repro replays a recorded path in a window and counts the stalls
`cargo xtask repro <frame-N-path.txt | bundle> --window [--real] [-- OPTIONS]` SHALL replay the camera path in real time in `planet editor --smoke` (the gate's path replay, TEST-061) over the world its bundle records: the seed and, when recorded, `--commit` and `--site` (VIEW-071). The options after `--` go to `planet editor` as they are. It SHALL print the gate line with the stalls (frames of the stall limit, 1 s, or more with the GPU phases counted, and the longest) and a verdict line. With `--real` a locked desktop SHALL end it as NOT RUN, never as a pass. It SHALL refuse a bundle that is not a terrain bundle, a missing path file, and a combination with `--at-commit`, `--every` or `--store`.

Verify: A
Status: active
Source: Lead 2026-11-13 (P2 E3), crusader's #124 diagnosis (no windowed path replay), VIEW-026, TEST-061

#### Scenario: The stalls of a path
- **WHEN** the gate line of a replay reads "stalls 7 (frames of 1000 ms or more with the GPU phases, longest 6400 ms)"
- **THEN** the verdict says 7 stalls and the options of the world the run opened

#### Scenario: A render submit counts
- **WHEN** a frame spends 4.5 s in its render submit and nothing else is slow
- **THEN** it is a stall of the report and not a failure of the gate's frame limit
