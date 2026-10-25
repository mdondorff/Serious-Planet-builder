## Why
Part (2) of `m8-relief` (ADR 0080 point 4, crusader's lane): the coupled run's constant uplift of 1e-3 m/year balances to a slope of `U / (K A^m)`, so every drained surface is driven toward 0.1 degree whatever the bake held (run minus base: -164 m to -4649 m medians over 500 m ground). The owner put plausible relief into M8; the run must keep the relief of its input.

## What Changes
- `coupled::stationary_uplift(lat, z, erodibility, params, rho)`: per land cell `rho K A^m S`, with the area and slope to the receiver taken from the input heights by the run's own flood, jittered receivers and accumulation; 0 at sea, at outlets and in pits. At `rho` 1 the input is a steady state of the implicit update.
- `coupledworld::bake_input` builds the run's uplift this way (`UPLIFT_RHO` 1) and the constant `UPLIFT_M_PER_YEAR` is gone. `rho` and the version of the field (`UPLIFT_FIELD_VERSION`) enter the declared identity of the inputs, so every stored run re-keys (the shared rebuild).
- Spec GEN-059.

## Impact
- Specs: generator-pipeline (GEN-059).
- Code: `generators/coupled.rs`, `generators/coupledinput.rs`, `app/coupledworld.rs`.
- Stored coupled runs (and the coarse horizon keyed through them) are unreachable under the new key: the shared rebuild makes them again. Nothing else changes (`run` and its checksums are unchanged).
- Out of scope: the hill band in the bake (separate change), the measurement of run minus base (after the rebuild).
