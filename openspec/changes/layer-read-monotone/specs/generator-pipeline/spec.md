## ADDED Requirements
### Requirement: GEN-188 The erosion layer's runtime read is continuous across cell-centre lines and exact at the cell centres
The runtime read of the erosion layer (`ErosionLayer::change_at`) SHALL be a monotone C1 cubic of the cells around the point along each axis: along each of the four rows, then across the rows, each the Hermite cubic through its two middle samples with the harmonic mean of the neighbouring differences as the slope at a sample when they have the same sign, else 0. The read SHALL stay within the range of the four cells around the point without a clamp, SHALL have no step across a cell-centre line (at most 1e-3 m across any line at 1e-7 cells on either side), and SHALL be the stored sample bit for bit at a cell centre (a cell coordinate within `CENTRE_SNAP_CELLS`, 1e-9 cells, of a centre is that centre). `LAYER_READ_VERSION` is part of the keys of a field that reads a layer (2 from this requirement on).

Verify: A
Status: active
Source: Lead 2026-11-13 (order #110, under P2 E1), the #108 diagnosis of F9 frame-13715 (the four-cell clamp stepped at cell-centre lines: 1.5 % of crossings over 0.1 m on R3a, up to 12.5 m), ADR 0122 (detail pinned to the stored samples)

#### Scenario: No step at a cell-centre line
- **WHEN** a layer with cell-scale noise of 25 m is read on both sides of a cell-centre row or column, 1e-7 cells apart, at 20,000 random crossings away from the cube edges
- **THEN** no pair differs by more than 1e-3 m

#### Scenario: The sample at a cell centre
- **WHEN** the layer is read at the direction made from a cell's indices
- **THEN** the read is the stored sample (f32 widened) bit for bit

#### Scenario: No overshoot
- **WHEN** the layer is read at random points away from the cube edges
- **THEN** every read lies within the range of the four cells around the point
