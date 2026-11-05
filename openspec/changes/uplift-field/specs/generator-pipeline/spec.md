## ADDED Requirements
### Requirement: GEN-123 The uplift field
An analytic uplift field SHALL give, at a direction, the uplift of a carve step in units of `k_dt` (the ratio `U / K` at K 1): CPU-canonical (`libm`, integer hashing), never stored, the same bits for any thread count, keyed by `UPLIFT_VERSION`. It SHALL be high in the active belts and the arc coasts, medium in the back-arc, about 0 on the cratons, from the province lattice of GEN-116 and a low-frequency noise. The face carve SHALL take it in place of the uniform ratio, scaled by the coastal taper only, not by the carve factor or by K; without it the carve SHALL give today's bits; `BakeInputs::id` SHALL fold its version and table.

Verify: A
Status: planned
Source: planner inbox #70 (owner, 2026-11-05), ADR 0102, GEN-116

#### Scenario: The field in the carve
- **WHEN** the field is built on 1 and 8 threads and read at random points and across a face edge, and a face is carved with it, with the uniform ratio, and without it
- **THEN** the bytes agree for any thread count, the value is highest in a belt and lowest on a craton, it is continuous across a face edge, the carve without the field is bit-identical to today's, and the keys with and without the field differ
