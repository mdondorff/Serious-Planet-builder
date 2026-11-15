## Why
- The owner's external review (2026-11-15, F3): `k7::surface_gate` (G-SURF) read the stored change `L` with the retired clamped Catmull-Rom, while the runtime reads it with GEN-188's monotone cubic. So G-SURF-I did not measure the surface the runtime draws.
- Lead, 2026-11-15 (#151, DEC crusader-5): the gate calls the runtime's read; `coast_fade` stays out (it waits on the owner's decision with D-2).

## What changes
- `erosion_layer::monotone_read`: the runtime's 4 x 4 read, extracted from `ErosionLayer::read_face` with the same operations in the same order, so the runtime's bits do not change. K11 and the gate both call it.
- `k7::gate_read`: the gate's read of `L` is the runtime's read; the bake's own surface keeps its clamped C1 read as the reference.
- `SurfaceGate`:
  - `max_unclamped_l_quantised_m` becomes `max_retired_c1_quantised_m`, `B` plus the retired clamped Catmull-Rom read of `L`, kept for comparison;
  - `max_l_overshoot_m` is removed, since the runtime's read has no clamp.
- GEN-148 and GEN-154: the deltas name the runtime's read.
- The test `bake_k7::the_surface_gate_reads_a_field_as_the_runtime_draws_it` (tier A) fails on main: on the review's trench the gate read -12.1875 m where the runtime draws -12.0900 m.

## Quarantine (lead, 2026-11-15)
- `bake_k7::the_runtime_surface_is_the_bake_surface_at_the_centres_and_within_a_step_at_the_lattice_nodes` now fails with its assertions unchanged.
- The runtime's read of `L` flattens trenches: on its smooth fixture tile the vertex difference is 0.48 m (bound 0.1 m) and 0.51 m quantised (bound 0.25 m).
- It is quarantined with a Tracked entry in docs/status.md. The fix is at the source, the runtime read (GEN-188); the owner decides the look.

## Expected golden changes
None: the runtime's read keeps its bits. The gate is a bake diagnostic and draws nothing.

## Impact
generators: `erosion_layer.rs` (an extraction), `bake/k7.rs` (the gate's read), the `k7_region` example (its print), `bake_k7` tests, and the G-SURF and G-SURF-I rows of docs/gates/ledger.md.
