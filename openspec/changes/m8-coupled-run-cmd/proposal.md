## Why
The M8-R rebuild (m8-relief, rebuild plan) makes the coupled run of the 16384 bake. Today only the viewer makes it (on a thread, no progress during the reduction of the bake, which reads about 25 million stored samples). The first make took 9,916 s; the time since GEN-065 (fill only) is unknown, and the projection must be known in the first minutes (opus-planner, 2026-10-25: over about 1.5 h stops the rebuild).

## What Changes
- `generate --coupled-run DIR --global-res N [--world F | --seed S]` makes the coupled run of the stored bake and stores it (the explicit step beside `--coarse-horizon`), printing the stages and the wall time.
- The reduction of the bake reports its bands done (`reduce_bake_with`); the run prints a line at most every 10 s with the share done and the projected total.

## Impact
- Spec: STRM-051; tier A test (the progress callback sees every band once, in a count that ends at the total; the result equals `reduce_bake`).
- Code: `generators/coupledinput.rs`, `app/coupledworld.rs`, `app/lib.rs`, `app/skeleton.rs`.
