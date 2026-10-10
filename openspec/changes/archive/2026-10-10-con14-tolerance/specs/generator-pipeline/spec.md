## MODIFIED Requirements

### Requirement: GEN-008 One implementation per layer
Within one cache each field layer SHALL be produced by exactly one implementation, and neighbouring tiles SHALL share one `implementation_id`. The CPU implementation SHALL be the canonical reference, and a GPU implementation SHALL agree with it per field within `|Δ| ≤ max(rel·|v|, abs)`: heights rel 1e-6, abs 1 mm; normals 0.01°; masks abs 1e-4.

Verify: B
Status: planned
Source: CON-07, CON-14, ADR 0010

#### Scenario: Differential
- **WHEN** a GPU implementation is dispatched on test inputs
- **THEN** its read-back output matches the CPU output within the field's bound, and a failure reports the field, the tile, the sample and both values

#### Scenario: No mixed seams
- **WHEN** a cache holds tiles of one layer
- **THEN** every tile and its neighbours carry the same `implementation_id`
