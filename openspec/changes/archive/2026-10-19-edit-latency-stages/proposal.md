## Why
Window 13 (2026-10-19, `docs/perf/window-13.md`): every edit-latency case is over its budget (preview 74 to 247 ms against 16 ms; exact 385 to 1283 ms) and the scene gives two numbers per case, so the cause cannot be read from it. opus-planner (2026-10-19): per-stage timestamps for the preview path and the exact path, then pick the fix from the numbers.

## What Changes
- The `edit-latency` scene records, per run, the stages of the edit (TEST-041): the durations of the three calls (`set_definition`, `ManagedLoop::edit`, `set_preview` with `preview_delta`), the instants (ms from `set_definition`) at which the first plan after the edit was delivered (with the worker's own time), R was delivered, the first tile job was handed to the spawner, half and all of the refresh requests had completed, and the new tiles were written (nothing waiting for an upload), and the median wall time of the frames of the run.
- A second table (`stages`) is printed after the budget table and written to `edit-latency.md` with it; the budget table and its verdicts are unchanged.

## Impact
- Specs: TEST-041 (added, testing).
- Code: `app` (`editperf.rs`: `Stages`, `stages_table`).
- Goldens: none; no budget changes. Perf: measurement only, no effect on the loop.
