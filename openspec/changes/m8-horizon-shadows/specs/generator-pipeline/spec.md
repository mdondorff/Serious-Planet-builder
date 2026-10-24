## ADDED Requirements

### Requirement: GEN-044 The chunk horizon map
`generators` SHALL bake, for a raster of heights with its halo, the horizon map of the chunk: per texel and per azimuth `k` of `A` evenly spaced azimuths from north, the elevation angle of the highest terrain along that azimuth out to the range, found by marching with steps that grow 4.5 % plus 2 m. The defaults are 4 m texels, `A` = 16, a range of 5 km and steps of 2 m plus 4.5 % of the distance; each is a parameter. Terrain at distance `d` SHALL count `d^2 / (2 R)` lower for a body of radius `R`. The angle is stored as a byte, 90 degrees over 255 steps, 0 at or below the horizon. Azimuths are in the raster's frame (`i` east, `j` north, azimuth 0 along `j`); the raster of a chunk on another cube face is rotated against its neighbour's, so GEN-046 maps the sun's azimuth into the chunk's frame and a texel shared across a face edge is compared after that mapping. A block with less than `ceil(range / texel) + 2` texels of raster on any side, a raster whose size does not match, and parameters that are not finite and positive SHALL be refused with the cause named. The bake SHALL be a pure function of the raster, the halo and the parameters, single-threaded per texel row set in a fixed order or independent of the thread count (CON-14), with `libm`, and texels shared by neighbouring chunks SHALL be identical.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), SPIKE-32 step 2, ADR 0077

#### Scenario: Known horizons
- **WHEN** the map is baked on a flat raster with one ridge of known height and distance
- **THEN** texels looking toward the ridge have the angle `atan(height / distance)` within the step and texel error, texels looking away from it have horizon 0, the result is equal for every thread count, a block baked from a raster that holds its halo equals the same block of a larger raster, and a raster without the halo changes the edge texels

### Requirement: GEN-045 The coarse horizon from the global run
`generators` SHALL compute, from the stored heights of the coupled run (STRM-043), the coarse horizon of every lattice cell: per cell and per azimuth the elevation angle of the highest cell along that azimuth out to the far range (about 50 km), by marching over the lattice through directions on the sphere. The result SHALL be a pure function of the stored heights and the parameters and be stored as a global channel beside the others (STRM-045).

Verify: A
Status: planned
Source: opus-planner (2026-10-24, M8-5), SPIKE-32 architecture finding 2

#### Scenario: A far peak
- **WHEN** the coarse horizon is computed on a small lattice with one high cell
- **THEN** cells within range toward it have its elevation angle at the azimuths toward it, cells beyond the range have none, and a march across a face edge sees the cell as one on the same face does

### Requirement: GEN-046 The horizon of a point
The horizon elevation used at a point and an azimuth SHALL be the larger of the chunk map's and the coarse map's, each interpolated linearly between the two nearest azimuths; this combination and the interpolation SHALL be one CPU function that is the reference of the shader's (CON-07), with the soft edge of the shadow term stated in the spec.

Verify: A
Status: planned
Source: opus-planner (2026-10-24, M8-5), CON-07

#### Scenario: Combination
- **WHEN** a point has a chunk horizon of 3 degrees and a coarse horizon of 5 degrees at an azimuth, and the sun is 4 degrees and then 6 degrees high there
- **THEN** the point is in shadow at 4 degrees and lit at 6 degrees, and with the coarse horizon absent the chunk map decides alone
