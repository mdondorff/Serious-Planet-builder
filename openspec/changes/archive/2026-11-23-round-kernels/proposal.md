## Why
A3 (docs/design/terrain-target.md) §6.2 to §6.4 and inbox #98 step 1: the prototype's erosion solver is a round, one region-wide routing followed by per-catchment carves. Two kernel rules make a catchment's round a function of its key inputs: the multiple-flow-direction area clipped to the D8 catchment, and a canonical flood tie rule.

## What Changes
- `route::RoundRules { clip_mfd: MfdClip, canonical_flood }` and `route_rules`: the MFD area clipped to the D8 catchment (`Local`: foreign lower neighbours dropped from the weights and their sum; `ToReceiver`: the first wording, kept for the comparison) and `flood::priority_flood_with(.., canonical)`: every cell enters the heap at its filled level and cells are visited by (level, cell id), no FIFO queue.
- `CarveParams.round_rules` (default off, a key byte only when on) and `CarveParams.schedule` (`Region` or one catchment after the other; execution only, the result is the same bits, not in any key): the per-catchment carve of G-ROUND.
- `Grid::with_active`: a mask the kernels see (a catchment routed alone, for the measurement).
- GEN-141 added (generator-pipeline).
- A3 §6.3, §6.4 and the gate table corrected with the measurements: G-LOCAL's "routed alone" form is replaced by "the round is a pure function of its key inputs"; routing locality moves to G-PART (step 2).

## Impact
- Code: `generators::bake::{route, flood, carve, mod}`, `cache::layer_store` (key byte), `examples/round_probe`. Nothing in today's output moves: both switches default off.
- Pass 2 refuses the round rules (its local carve would run under a key that says otherwise).
