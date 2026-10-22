# Change: edit-scale-scene

## Why
- SPIKE-29 (owner via opus-planner, 2026-10-22): how far do authored edits scale? The generator and the measurements are kept as a reusable scene so the curve can be re-run after the fixes the spike ranks (`spikes/edit-scale/results/results.md`).

## What changes
- `crates/app/src/editscale.rs`: the scene (`test-render --scene edit-scale`), the seeded generator (`Rng`, `choose`, `place_of`, `make_one`), a counting field (`Timed`), the measurements; dispatched before the GPU context is made.
- `editperf.rs`: `PLANET_EDIT_ITEMS=DIR` loads a set into the definition of the edit-latency scene; `case_centre` is `pub(crate)`.
- TEST-045.

## Tests
- Tier A (`editscale.rs`): the sets are a function of the seed; the mix and the cluster share; the counting field forwards and counts.

Expected golden changes: none.
