## Why
SPIKE-84 (`spikes/b5-test-robustness/results/results.md`) found two configuration gaps behind the test noise:
- On CI's Windows WARP runner the edit-latency smoke scene (`gpu_edit_perf`, 22 failed runs in 14 days) and the stills scene (`gpu_stills`, 15) hit the 300 s hang limit under runner load and pass on re-run. Unlike the window flight, the coastal flight and the lit coast view (2026-10-29), they have no declared limit.
- A local software `test gpu` runs about 20 rasterising processes at once. The 12 local tier-C timeouts of 2026-10-20 and three `gpu_skeleton` timeouts on 2026-10-31 came from that load. `--real` is capped at 4; the software tier is not.

## What Changes
- `.config/nextest.toml`:
  - a declared 450 s limit (5 x 90 s) for the edit-latency smoke scene and the stills scene, each with its measured CI numbers in a comment;
  - `gpu_edit_perf`, `gpu_stills` and the three loop tests of `gpu_skeleton` (the ones that timed out) join the `heavy-window` group (at most 2 at once);
  - a new group `gpu-tier` of at most 8 for every other `gpu_*` test.
- No test is skipped, retried or changed; tier A and B are not capped; CI's 4-core runners are unaffected by the cap of 8.

## Capabilities
### Modified Capabilities
- `build-tooling`: BUILD-003, the hang-limit scenario (declared limits) and a new scenario (the software GPU tier's cap and the heavy group).

## Impact
- Config only: `.config/nextest.toml`. No code, no goldens.
- A local software `test gpu` may take a little longer and stops timing out under its own load.
