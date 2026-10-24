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
`generators::coarsehorizon` SHALL compute, from the stored heights of the coupled run (STRM-043), the coarse horizon of the cells of an observer lattice of `m` cells per face edge (a divisor of the run's): per cell and per azimuth the elevation angle (the chord above the observer's horizontal plane, on the sphere) of the highest sample of the run along the great circle in that azimuth, out to the far range (default 50 km, 16 azimuths, samples one cell of the run apart, at most 256 azimuths, a range of at most a quarter of the way round the body), where each sample is the run's height at the sample's direction (so a march crosses a cube face edge without face coordinates) and sea at or below 0 counts as 0; the observer's own height is the mean over the run cells its cell covers. The angle is the byte of GEN-044. Azimuths are in the geographic frame (0 toward the pole axis, a quarter turn east), unlike GEN-044's raster frame; GEN-046 rotates the sun's azimuth into each. The result is independent of the thread count, and a wrong size, a bad `m` or parameters that are not finite and positive are refused with the cause named. The result SHALL be a pure function of the stored heights and the parameters and be stored as a global channel beside the others (STRM-045).

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), SPIKE-32 architecture finding 2

#### Scenario: A far peak
- **WHEN** the coarse horizon is computed on a small body with one wide plateau on a cube edge, and on a lattice of varied heights with sea
- **THEN** cells within range see the plateau toward it at its elevation angle (between the angles of its far and near edges), cells on the other face see it across the edge, cells beyond the range see nothing, a flat body has no horizon, the result is the same for every thread count, and deeper sea changes nothing

### Requirement: GEN-046 The horizon of a point
`generators::horizon_lookup` SHALL be the CPU reference of the shader's shadow (CON-07). The horizon at an azimuth is read by linear interpolation between the two nearest stored azimuths (wrapping from the last to the first); the horizon of a point is the larger of the chunk map's and the coarse map's, a map that is not there (the chunk's has not landed) leaving the other alone and neither giving 0; the lit factor is 0 when the sun's elevation is 0.35 degrees or more below the horizon, 1 when 0.9 degrees or more above it, and a smoothstep between. The sun's azimuth SHALL be formed in each map's own frame: the geographic frame for the coarse map, the cube face's raster frame (`i` east, `j` north, which the warp shears by up to about 30 degrees) for the chunk's, where the sun's horizontal part is split into its components along the two axes. The shader SHALL equal this within the tolerance of its tests (REND-038).

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), CON-07

#### Scenario: Combination
- **WHEN** a point has a chunk horizon of 3 degrees and a coarse horizon of 5 degrees at an azimuth, and the sun is 4 degrees and then 6 degrees high there
- **THEN** the point is in shadow at 4 degrees and lit at 6 degrees, and with the coarse horizon absent the chunk map decides alone (and the coarse map alone before the chunk's lands), the ramp is monotone between its ends, the interpolation is exact at the stored azimuths and wraps, and the sun's azimuth is 0 toward north, a quarter turn toward east in both frames and differs between the two frames on a turned face
