## Why
The rebuild of 2026-10-25 (`m8-relief` part 2, GEN-063) did not meet the target "run minus base within +-100 m at river cells": the stationary uplift is a steady state only on the smooth synthetic world. On the real reduced bake the run drifts from its own input by -104 to -4471 m (median by band) over 80 steps and saturates near step 24; step 1 alone raises 4.24 of 7.37 million land cells by more than 1 m. The run's job for M8 is the river network and the areas, not erosion (planner's decision, 2026-10-25).

## What Changes
- `CoupledParams::erode` (in the key of the stored run). Off, `run` returns `filled_state`: the heights are the input plus the priority-flood fill, the receivers and areas are those of that surface, one pass (GEN-065).
- A world's run sets `coupledworld::COUPLED_ERODE` = false; `bake_input` then skips the stationary uplift. The stream-power steps and GEN-063 stay behind `erode` for M9's calibration; the algorithm's own default stays on, so every erosion known answer is unchanged.
- ADR 0080 point 4 rewritten; the unexplained mechanism is in `docs/learnings.md` (no further time on it now).

## Impact
- Spec: generator-pipeline (GEN-065; GEN-063's use is conditioned on `erode`).
- Code: `generators/coupled.rs`, `cache/global_store.rs` (the key), `app/coupledworld.rs`.
- Every stored coupled run and the coarse horizon re-key: the coupled run and the horizon are made again on the shared store (the bake stays).
