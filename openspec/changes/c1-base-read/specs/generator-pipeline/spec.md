## ADDED Requirements

### Requirement: GEN-107 C1 read of the bake and the field cpu-erosion-v2
A reader of directions SHALL give the bake's elevation by a C1 interpolant (`read_elevation_c1`): the separable Catmull-Rom blend (tension 0.5) of the 4 x 4 lattice samples around the position, in `f64` on the stored `f32` elevations, rounded to `f32` once. A lattice point SHALL read as its own stored sample, with the weight-zero nodes not read. Nodes beyond a face edge SHALL be the bilinear read (`read_direction`) of the direction of the extended face coordinates, that is the neighbour face's own samples (the third face's at a cube corner). The read SHALL be a pure function of the direction, so a point on a face edge or at a cube corner reads the same bits whichever adjacent face's coordinates it was given in, on any thread count and in any batch. Across a cell edge inside a face the slope SHALL be continuous (its jump far below the bilinear read's); across a face edge the jump SHALL be reduced, not removed. The reader's version (`READER_C1_VERSION`) and known answers SHALL be pinned.

The field `cpu-erosion-v2` (`HeightFieldKind::ErodedC1`, selected by `world.height_field`) SHALL be the eroded field of GEN-017 with its base read by `read_elevation_c1` (also in the slope stencil), under its own implementation id and version (`EROSION_C1_VERSION`), with a key family apart from `cpu-erosion-v1`, which SHALL stay unchanged. Its `height_bounds` SHALL contain the height of every direction of the cap (also rounded to `f32`): the scanned window SHALL grow by the stencil and the neighbour ghosts, the sample range by 0.28125 of itself on each side (the overshoot of the blend), and the largest lattice step by 4 in the slope bound. The determinism manifest SHALL list the field's heights at every face's edges and corners.

Verify: A
Status: active
Source: Owner 2026-11-03 (pyramid blocks), inbox #50, E2b probe, ADR 0027, ADR 0028

#### Scenario: Lattice points
- **WHEN** lattice points on faces, edges and corners are read
- **THEN** each reads the bits of its stored sample

#### Scenario: Slope continuity
- **WHEN** the slope jump across cell edges inside faces and across face edges is measured for the C1 and the bilinear reads
- **THEN** the C1 mean jump inside a face is below 5 % of the bilinear's and across a face edge below 50 %

#### Scenario: Same bits from either face
- **WHEN** points on the 12 edges (including near the corners) and the 8 corners are read from the coordinates of each adjacent face
- **THEN** the directions with equal bits read equal heights, and the others differ by under a millimetre

#### Scenario: Bounds
- **WHEN** random caps, caps on edges and corners, and caps over a lattice with spikes and steps are bounded
- **THEN** every sampled height, also rounded to f32, lies within the bounds, and the blend's overshoot stays below 0.28125 of the sample range

#### Scenario: Determinism
- **WHEN** heights are read singly, as a gathered batch and on 1, 2, 4 and 7 threads
- **THEN** all bits are equal, and the known answers and the versions are the pinned values
