## ADDED Requirements
### Requirement: GEN-188 The erosion layer's runtime read is continuous across cell-centre lines and exact at the cell centres
The runtime read of the erosion layer (`ErosionLayer::change_at`) SHALL be a monotone C1 cubic of the cells around the point along each axis: along each of the four rows, then across the rows, each the Hermite cubic through its two middle samples with the slope `2ab / (a + b) * 2ab / (a² + b²)` at a sample whose neighbouring differences `a` and `b` have the same sign, else 0. Away from the cube-edge blend (`EDGE_BLEND_CELLS`) the read SHALL stay within the range of the four cells around the point without a clamp (up to rounding: a few ulp), SHALL have no step across a cell-centre line (at most 1e-4 m across any line at 1e-7 cells on either side), SHALL have no kink inside a cell (a slope jump of at most 5 m per cell over 1e-4 cells on a layer with cell-scale noise of 25 m), and SHALL equal the stored sample at a cell centre, bit for bit for a non-zero sample (a cell coordinate within `CENTRE_SNAP_CELLS`, 1e-9 cells, of a centre is that centre). `LAYER_READ_VERSION` is part of the keys of a field that reads a layer (2 from this requirement on).

Verify: A
Status: active
Source: Lead 2026-11-13 (order #110, under P2 E1), the #108 diagnosis of F9 frame-13715 (the four-cell clamp stepped at cell-centre lines: 1.5 % of crossings over 0.1 m on R3a, up to 12.5 m), numerics review 2026-11-13 (a harmonic-mean slope kinked inside the cells), ADR 0122 (detail pinned to the stored samples)

#### Scenario: No step at a cell-centre line
- **WHEN** a layer with cell-scale noise of 25 m is read on both sides of a cell-centre row or column, 1e-7 cells apart, at 20,000 random crossings away from the cube edges
- **THEN** no pair differs by more than 1e-4 m

#### Scenario: No crease inside a cell
- **WHEN** the same layer is read along a 6-cell scan at 1e-4 cells, along x at a fixed fraction of a row and along y at a fixed fraction of a column, 60 scans
- **THEN** no second difference over the step exceeds 5 m per cell (the harmonic-mean slope alone gives about 15-20)

#### Scenario: The sample at a cell centre
- **WHEN** the layer is read at the direction made from a cell's indices, on a 256-cell face and on the production face of 98,304 cells
- **THEN** the read is the stored sample (f32 widened) bit for bit

#### Scenario: No overshoot
- **WHEN** the layer is read at random points away from the cube edges
- **THEN** every read lies within the range of the four cells around the point
