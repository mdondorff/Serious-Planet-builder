## ADDED Requirements
### Requirement: GEN-127 The carves read the erodibility and the uplift fields
Both carves SHALL take an optional rock: the erodibility `K(cell, z)` of GEN-116 and, for pass 1, the uplift `U(cell)` of GEN-123.
- In each step, before the sweep, the erodibility of every cell SHALL be evaluated at the cell's height at the start of the step (in parallel over rows, a value per cell: the same bits for any thread count) and SHALL multiply the class carve factor in that step's implicit update; it SHALL be finite and not negative.
- A given uplift SHALL replace `u_over_k`: a cell gains `k_dt * U * taper` in a step, the taper the smoothstep of the cell's starting height above the sea over `uplift_taper_m` as before; the uplift SHALL NOT be scaled by K or by the class carve factor (ADR 0103 point 3), and SHALL be finite and not negative.
- Pass 2's local carve (incise only, no uplift) SHALL read K only.
- With no rock, or with `K = 1` and `U = u_over_k` everywhere, the carve SHALL be bit for bit the carve without fields.
- `BakeInputs::id` SHALL fold `LITHOLOGY_VERSION` and `UPLIFT_VERSION` into the layer keys of a bake that reads the fields.

Verify: A
Status: active
Source: ADR 0102, ADR 0103, GEN-100, GEN-101, GEN-116, GEN-123; planner inbox #75

#### Scenario: The fields in the carve
- **WHEN** a cone is carved with K = 1 and U = `u_over_k`, with K 0.12, 1 and 1.8, with K 0 and the factor 1 and 5, with a K that records its reads, on 1 and 8 threads, with a negative K and a negative U, and a face is carved with direction fields on 1 and 8 threads
- **THEN** the first equals the carve without fields bit for bit, a harder rock settles higher, a K of 0 lifts each cell by exactly `steps k_dt U taper` whatever the factor, step 2 reads the heights that step 1 left, the bytes agree for any thread count, the negative inputs are refused, and the face's change differs from the one without fields
