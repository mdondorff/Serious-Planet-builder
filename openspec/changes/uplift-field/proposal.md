## Why
- The bake lifts every cell by the same ratio `U / K = 20` (tapered at the coast): a craton rises as fast as an orogen, so relief comes only from the base elevation and the class factor. Owner (2026-11-05, planner inbox #70 item 1): the uplift becomes a field, high in the active belts, medium in the back-arc, about 0 on the cratons, positive on the arc coasts. It is an input of the bake like the lithology (ADR 0102), so it comes before the planet bake and with a preview.

## What Changes
- GEN-123: an analytic uplift field `U(d)` (the ratio `U / K` at K 1), CPU-canonical (`libm`, integer hashing, same bits for any thread count), never stored, keyed by `UPLIFT_VERSION`. It reads the province lattice of `lithology-k` (the class source's plate-history terms), which gains the back-arc term, and a low-frequency noise.
- The face carve takes the field in place of the uniform `u_over_k`: the per-step uplift of a cell is `k_dt * U(d) * taper`, and it is not scaled by the carve factor or by K, so that rock of different hardness settles at different slopes (steady state `S = U / (K sqrt(A))`).
- A planet preview (`generate --uplift-preview DIR`): maps of `U`, of `U / K` at the surface (the relative steepness the carve will settle at), a section from a belt over a back-arc to a craton, and `uplift.txt`. Every image says it shows an input, not terrain.
- Keys: `UPLIFT_VERSION` and the table fold into the bake inputs' identity (`BakeInputs::id`), so every face layer key changes; no stored data is rewritten (the read-only stores are never written; the preview store is already stale).

## Impact
- Code: `generators` (`uplift.rs`, `lithology.rs` back-arc term and `LITHOLOGY_VERSION` 2, `bake/carve.rs`, `bake/face.rs`, `bake/inputs.rs`), `cache` (layer key), `app` (the option).
- Relief changes everywhere the bake runs: the class amplitudes and the scorecard are re-tuned after it, together with ADR 0102's K (the preview round).
- All numbers are start values for the owner's preview round.
