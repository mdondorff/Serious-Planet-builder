## ADDED Requirements
### Requirement: GEN-187 Where the layer is read the fine detail is pinned to the stored samples
Where the erosion layer is read (GEN-112) the field's fine-detail term (ADR 0098) SHALL be the detail minus its bilinear interpolant from the four layer cell centres around the point, 0 at a centre, so the drawn ground equals the stored base plus the faded change at every cell centre, bit for bit. Within two cells of a cube edge the interpolant is cross-faded with the neighbouring face's, so the term has no step there. The height bounds SHALL hold the pinned detail, a batch SHALL read the centres as single reads do, and the pinning's version SHALL enter the keys of every field that reads the layer.

Verify: A
Status: active
Source: ADR 0122 point 1 (accepted, owner 2026-11-13), SPIKE-113, VIEW-072's probe (the drawn ground minus B + L on R3a: p50 -2.1 m, all of it the detail)

#### Scenario: A cell centre
- **WHEN** the field with the layer is read at the centre of a layer cell more than two cells from a cube edge
- **THEN** the height is the base elevation plus the coast-faded change there, bit for bit

#### Scenario: A cube edge
- **WHEN** the field with a layer of zeros is read at pairs of points 0.13 mm apart across every cube edge and corner
- **THEN** no pair differs by a millimetre

#### Scenario: A batch over a blocked source
- **WHEN** sparse, dense, edge and cross-face batches are read over a source with blocks
- **THEN** every height is finite and equals the single read, and no error is recorded

#### Scenario: Between the centres
- **WHEN** it is read between centres
- **THEN** the detail is the unpinned detail minus its interpolant from the four centres, a batch reads as single reads, and the height lies within the bounds of every cap that holds the point
