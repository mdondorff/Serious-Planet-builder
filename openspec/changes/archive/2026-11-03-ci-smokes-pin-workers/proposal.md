## Why
Since #1117 the default tile pool is the machine's terrain share. On a 4-thread CI runner that is one tile worker; it used to be two. CI smokes with their own wall-clock limits started to time out:
- #1130's lavapipe C: `gpu_stream_perf::the_six_scenes_run_end_to_end_on_a_stored_bake_in_smoke_mode_and_log_their_source`, "did not finish in 240s";
- #1104's lavapipe C, the same;
- #1117's `gpu_edit_perf` no-settle earlier (Tracked).
STRM-020 already says that a run that must be reproducible (CI, goldens, a perf comparison) pins `--gen-workers`, and these did not (opus-planner, 2026-11-03).

## What Changes
- The CI smokes that run the streamer under a wall-clock limit of their own pin `--gen-workers 2`, the pre-#1117 effective size on a 4-thread runner, citing STRM-020: `gpu_edit_perf`, `gpu_edit_pair` (2 runs), `gpu_envelope_perf`, `gpu_lit_stream_flicker`, `gpu_lit_still_flicker` and `gpu_still_flicker`. These are the tests that already set `--gen-priority normal` for the same small runner.
- `gpu_stream_perf`'s `run` helper pins two workers on every `test-render` run that does not size the pool. The reserve-report smoke, which judges the machine's terrain share itself (BUDG-003), runs unpinned through `run_as_given`.
- Not pinned: `gpu_window`, `gpu_skeleton`, `gpu_flight`, `gpu_stills` and the tier-A/B world tests. They have no scene wall-clock limit, only nextest's timeout, or do not stream at the default share.

## Impact
- Tests only; no product code or spec change (STRM-020 already states the rule).
- The pinned runs log "pool: override 2 (share S)" and mark their frame verdicts as not at the BUDG-003 share. Smokes are never measurements, so nothing is judged on them.
