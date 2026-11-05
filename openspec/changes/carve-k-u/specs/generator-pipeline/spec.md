## ADDED Requirements
### Requirement: GEN-127 The carves read the erodibility and the uplift fields
Both carves SHALL take an optional rock: the erodibility `K(cell, z)` of GEN-116 and, for pass 1, the uplift `U(cell)` of GEN-123.
- The rock SHALL give, with K, the height of the lower edge of the layer the cell's height lies in; a step SHALL NOT lower a cell below that edge by more than `LAYER_SKIP_M` (0.01 m), the rest is carried to the next step, which reads the layer below (a trunk's one step is thicker than a band, and would otherwise skip a caprock).
- In each step, before the sweep, the erodibility of every cell SHALL be evaluated at the cell's height at the start of the step (in parallel over rows, a value per cell: the same bits for any thread count) and SHALL multiply the class carve factor in that step's implicit update; it SHALL be finite and not negative. The lithology's K SHALL be normalised (`BakeInputs::rock_k`) by the factor that makes its mean over the land of a fixed equal-area sample (`K_NORM_SAMPLES` directions, read at the base elevation) 1, so that the rock varies the erosion in space and the carve keeps its overall rate (planner inbox #79); the sample is part of the key (`BakeInputs::id`).
- A given uplift SHALL replace `u_over_k`: a cell gains `k_dt * U * taper` in a step, the taper the smoothstep of the cell's starting height above the sea over `uplift_taper_m` as before; the uplift SHALL NOT be scaled by K or by the class carve factor (ADR 0103 point 3), and SHALL be finite and not negative.
- Pass 2's local carve (incise only, no uplift) SHALL read K only.
- With no rock, or with `K = 1` and `U = u_over_k` everywhere and no carve factor (or a factor of 1), the carve SHALL be bit for bit the carve without fields; a given erodibility without an uplift array SHALL NOT scale the uniform uplift (only the class factor does).
- `BakeInputs::id` SHALL fold `LITHOLOGY_VERSION` and `UPLIFT_VERSION` into the layer keys of a bake that reads the fields.

Verify: A
Status: active
Source: ADR 0102, ADR 0103, GEN-100, GEN-101, GEN-116, GEN-123; planner inbox #75

#### Scenario: The fields in the carve
- **WHEN** a cone is carved with K = 1 and U = `u_over_k` (no factor), with K 0.12, 1 and 1.8, with K 0 and the factor 1 and 5, with a K that records its reads, with a caprock over soft rock on a steep ramp with and without the edge, on 1 and 8 threads, with a negative K and a negative U, and a face is carved with direction fields on 1 and 8 threads
- **THEN** the first equals the carve without fields bit for bit, a harder rock settles higher, a K of 0 lifts each cell by exactly `steps k_dt U taper` whatever the factor, step 2 reads the heights that step 1 left, a caprock forms a step (no cell passes its top in one step, while the same carve without the edge lowers cells through it), the bytes agree for any thread count, the negative inputs are refused, and the normalised erodibility has a land mean of 1 on another sample, and the face's change differs from the one without fields
