## ADDED Requirements

### Requirement: GEN-070 The drivers of a chunk
`generators::drivers` SHALL compute, for every core cell of a chunk raster from the cell and its eight neighbours, the slope, the wetness `ln(A / (w tan s))` with `tan s` floored at 0.01 and the drainage area `A` clamped to 1e8 m2, the soil depth (the SPIKE-30 stand-in, 0 to 8 m) and the annual-mean insolation of the cell's latitude, slope and aspect (never the current sun). The functions SHALL be pure, use `libm` only and no threads, and their constants SHALL be part of the stage's definition hash.

Verify: A
Status: planned
Source: ADR 0076 points 2 and 3, SPIKE-30, planner decision 2026-10-26

#### Scenario: Known rasters
- **WHEN** the drivers are computed on a plane, a cone and a hollow whose area exceeds the clamp
- **THEN** the slope and curvature equal the geometry's, the wetness of the hollow is the maximum, and a flat gives the floor's value, at 1 and 8 threads alike

### Requirement: GEN-071 Driver channels of a chunk and their blend
A chunk result SHALL carry the soil depth, the moisture and the insolation as three 8-bit channels at the raster with box-filtered pyramid levels, in the chunk's disk record. A consumer SHALL read a driver as `neutral + w (chunk - neutral)` with `w` the chunk weight that fades the height; the neutral values are soil 1 m, moisture 0.5 and the insolation of the latitude alone.

Verify: A
Status: planned
Source: ADR 0076, GEN-037, planner decision 2026-10-26

#### Scenario: The range edge
- **WHEN** the weight of a chunk goes from 0 to 1
- **THEN** the driver goes from the neutral value to the chunk's value, monotone, with no step

### Requirement: GEN-072 The materials read the drivers
`MaterialParams::weights` SHALL read the soil depth, the moisture and the insolation from the blended drivers in its reserved inputs, by the same function and the same input vector as for the other inputs.

Verify: A
Status: planned
Source: LOOK-008, ADR 0076 point 5

#### Scenario: A hollow
- **WHEN** a look gives soil a curve on the moisture input and the drivers describe a wet hollow
- **THEN** soil takes weight in the hollow and none on the dry flank

### Requirement: GEN-073 Neutral drivers change nothing
With the neutral drivers (no chunk landed, or weight 0) the material weights, the albedo and the images of every committed golden SHALL be bit-identical to those of a build without the driver inputs.

Verify: A
Status: planned
Source: planner decision 2026-10-26, ADR 0017

#### Scenario: Today's goldens
- **WHEN** the committed goldens are rendered with the driver inputs present and neutral
- **THEN** every image equals its golden bit for bit
