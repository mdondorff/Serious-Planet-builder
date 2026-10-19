## ADDED Requirements

### Requirement: VIEW-026 A camera path replays through the plan and F9 says it wrote
`cargo xtask repro <frame-n-path.txt>` SHALL replay the path with its bundle: `test-render --repro BUNDLE --replay-path PATH --out-dir DIR [--replay-every N]` SHALL plan the terrain of a terrain repro bundle (a world, or the seed-only field) once per pose of the path file, in the logged order, with the bundle's lens, view and LOD parameters, and SHALL write `DIR/replay.txt` (`t_ms altitude_m nodes plan_hash finest_level plan_ms logged_frame_ms`, one line per pose) and, with `--replay-every N`, `DIR/pose_NNNN.png` for poses 0, N, 2N and so on. A pose the plan refuses (a terrain too steep for the bundle's LOD parameters at that camera) SHALL be written as a `REFUSED` line with the reason and the replay SHALL go on. It SHALL print the number of poses (refused ones included, with their reasons), the least and most nodes, the finest level, the slowest plan, and the poses where the finest level fell while the altitude fell, each with its time and altitude. `--replay-path` without `--repro` or `--out-dir`, a path file that does not parse (VIEW-024) or holds no pose, and a bundle that is not a terrain bundle SHALL be refused naming the problem. `cargo xtask repro <bundle>` SHALL name the `frame-<n>-path.txt` beside it when there is one. F9 SHALL show what it wrote (the two file names, the pose count and the span, the directory) in the window title for 8 s and print it with `PathLog::summary()`, and SHALL show a failed write the same way.

Verify: A, C
Status: active
Source: opus-planner (2026-10-19), VIEW-024, TEST-008

#### Scenario: A level that drops on approach
- **WHEN** the finest level falls from 19 to 17 while the altitude falls from 800 m to 700 m, and falls from 17 to 16 while the altitude rises
- **THEN** the first pose is listed as a drop with its time and altitude, and the second is not

#### Scenario: Files name each other
- **WHEN** `repro` is given `target/review/frame-120-path.txt` or `target/review/frame-120.bundle`
- **THEN** the bundle is `frame-120.bundle` and the path file `frame-120-path.txt` in the same directory

#### Scenario: Replay of what the real window wrote
- **WHEN** the editor smoke wrote `frame-<n>.bundle` and `frame-<n>-path.txt` and the path is replayed against the bundle
- **THEN** `replay.txt` holds one line per pose and the summary names that many poses
