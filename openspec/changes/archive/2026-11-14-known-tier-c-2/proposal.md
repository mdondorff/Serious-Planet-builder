## Why
- Lead, 2026-11-14 (after #1515): today's local tier C also timed out in `gpu_skeleton::the_draw_flag_selects_the_path_and_every_loop_draws_the_same_frame_on_either`; and the list's gpu_preflight class covered line 22, which asserted the refusal text and the 8 s bound together.

## What changes
- `xtask/known-tier-c.txt`: a timeout line for the draw-flag test. Evidence: crusader's local log `test-gpu-20261114-151534.log` (TIMEOUT at 450.138 s, the only timeout beside three WARP aborts); `spikes/gpu-skeleton/results/results.md` (#1442) diagnoses exactly this test: 15.3 s alone, its two `--pipelined` loops slow only under CPU oversubscription on WARP. The 14:08 log (`test-gpu-20261114-140842.log`) has no timeout (one WARP abort).
- `crates/app/tests/gpu_preflight.rs`: the refusal's assertion split in two; the text (line 23) and the 8 s bound (line 24, its own message). Every property assertion stays; the list's class names line 24 only, so a wrong refusal text fails prepush.
- BUILD-056's scenario and source; docs/status.md's Tracked entry names line 24.

## Impact
xtask list and tests, one tier-C test file (no tolerance changes).
