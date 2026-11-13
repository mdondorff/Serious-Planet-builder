## ADDED Requirements
### Requirement: GEN-187 Where the layer is read the fine detail is pinned to the stored samples
Where the erosion layer is read (GEN-112) the fine-detail term of the field (ADR 0098) SHALL be the detail minus its bilinear interpolant from the four layer cell centres around the point, and 0 at a cell centre, so that the drawn ground equals the stored base plus the faded change at every cell centre, bit for bit. The detail at the centres is the same term evaluated there. The height bounds SHALL hold the pinned detail (the detail over the cap widened by two layer cells for the interpolant), and the version of the pinning SHALL enter the keys of every field that reads the layer. Where no layer is read the field is unchanged.

Verify: A
Status: active
Source: ADR 0122 point 1 (accepted, owner 2026-11-13), SPIKE-113, VIEW-072's probe (the drawn ground minus B + L on R3a: p50 -2.1 m, all of it the detail)

#### Scenario: A cell centre
- **WHEN** the field with the layer is read at the centre of any layer cell
- **THEN** the height is the base elevation plus the coast-faded change there, bit for bit

#### Scenario: Between the centres
- **WHEN** it is read between centres
- **THEN** the detail is the unpinned detail minus its interpolant from the four centres, a batch reads as single reads, and the height lies within the bounds of every cap that holds the point
