## Why
#1114's C 2/3 failed `gpu_lit_stream_flicker` with "did not settle after 264 ticks in 302 s: 0 in flight, 0 queued, 1292 resident, EV 15.89". The message left open whether streaming was done and only the loop was still busy, or whether the wall clock stopped a slow but progressing settle. A quiet tick also needs an Exact draw, no deferred upload and an idle plan worker, and the message printed none of these (opus-planner, 2026-11-03).

## What Changes
- `settle_error` (crates/app/src/streamperf.rs) formats the lit-stream settle's error. It names:
  - the run of quiet ticks reached, against the run needed;
  - the limit that stopped it: the 900-tick cap or the wall clock;
  - each condition of a quiet tick in the last frame: the draw kind, in flight, queued, deferred, the plan worker busy or idle;
  - as before, resident and EV.
- `Tick` carries the frame's draw kind and the plan worker's busy flag.
- No limit changes: the 300 s smoke and 900 s wall-clock limits and the 900-tick cap stay.

## Impact
- `crates/app/src/streamperf.rs` only. There is a tier-A test of the message (TEST-070); there are no spec deltas, because the constitution's actionable-failure rule already covers it.
