## Why
- In #1108's run (41686252144, attempt 1), `windows (WARP) C 2/4` cut off `gpu_skeleton::the_draw_flag_selects_the_path_and_every_loop_draws_the_same_frame_on_either` at 300.26 s, the hang limit of the `gpu_` tier (BUILD-003).
- Beside it ran the water coverage (189 s), the lit stream scene (157 s), the edit pair (175 s) and the editor window (100 s). Two of those four are `warp-heavy`.

## Measurement
CI durations of the test on 2026-11-03 (12 runs, main and PRs):

| Runner | Min | Max | Note |
|---|---|---|---|
| Windows WARP, `C 2/4` | 57.5 s | 199.7 s | 57 s when it started before the heavy tests, 140-200 s beside them |
| Linux lavapipe | 8.1 s | 16.7 s | |

What the test does: five `test-render` runs of the aerial-200km relief view, each on its own device.

| Run | Frames | Local time (software adapter, this machine's state) |
|---|---|---|
| per-draw | 1 | 1.6 s |
| indirect | 1 | 1.9 s |
| indirect, streamed | 1 | 1.7 s |
| indirect, pipelined | 20 (cover 1, stale 18, exact 1) | 2.7 s |
| per-draw, pipelined | 20 | 2.5 s |

- Almost all of each run's cost is per run (the field, the device, its pipelines). The 19 extra pipelined frames add about 0.9 s.
- Fewer frames would cut perhaps 15-20 % and leave the load swing as it is. Each run proves a distinct path, so none can go.

## What Changes
- The test joins `warp-heavy` (moved from `heavy-window`): it no longer runs beside the window, flight and lit-streaming members.
- It gets a declared 450 s limit (5 x 90 s) with its durations in `.config/nextest.toml`, like the stills tests. It still catches a hang.
- The xtask config test guards both: the limit comes before the `gpu_` override, the test is in `warp-heavy`, and `heavy-window` no longer names it.
- BUILD-003 modified: the scenario lists of declared limits and of the two heavy groups.
- No test is skipped, retried or changed.

## Impact
- `.config/nextest.toml`, `xtask/src/testcmd.rs`; BUILD-003 modified.
- The `warp-heavy` chain of `C 2/4` grows by this test's alone time: about 60 s, from today's 157 s lit stream scene and 100 s editor window.
