## ADDED Requirements

### Requirement: GEN-089 Local terrain descriptors
`planet_generators::descriptors` SHALL compute, on a height raster of cell size `h` with marked no-data cells, the slope in degrees (`atan` of the central-difference gradient) and the profile and contour curvature in 1/m from the Evans-Young quadric fit over the 3x3 window. Profile curvature is taken along the gradient and contour curvature across it, both positive where the surface is convex. A cell whose window touches the edge or a no-data cell SHALL be uncounted. The computation SHALL use f64 and `libm` and give the same bytes at any thread count (CON-14).

Verify: A
Status: planned
Source: SPIKE-57 (scorecard, descriptors survey CGF 44(2) 2025), SPIKE-55, CON-14

#### Scenario: Plane and quadrics
- **WHEN** the kernels run on a plane rising 10 degrees east, and on `z = a (x^2 + y^2)` and `z = a x^2` sampled at `(x0, 0)`
- **THEN** the slope is 10 degrees at every counted cell within 1e-9, the profile curvature is `2a / (1 + 4 a^2 x0^2)^(3/2)` on both within 1 %, and the contour curvature is `1 / x0` on the paraboloid and 0 on the trough

#### Scenario: Threads
- **WHEN** the kernels run on the same raster with 1 and 8 threads
- **THEN** the outputs are byte-identical

### Requirement: GEN-090 Relief and position descriptors
The descriptors module SHALL compute the local relief (max minus min) over non-overlapping square blocks of a given side, and the topographic position index (TPI): the height minus the mean over the square window of side `2r+1` cells, from summed-area tables accumulated in f64 in row order. A window that touches the edge or a no-data cell SHALL be uncounted. The results SHALL be thread-count independent (CON-14).

Verify: A
Status: planned
Source: SPIKE-57, SPIKE-55 (relief by blocks), CON-14

#### Scenario: Step and wave
- **WHEN** a raster holds a 100 m step through one block and a long sine wave
- **THEN** that block's relief is 100 m and a flat block's is 0, the TPI is positive on the crests and negative in the troughs, and the summed-area mean equals the direct mean within 1e-9

### Requirement: GEN-091 Geomorphons
The descriptors module SHALL classify each counted cell into one of ten landform classes (flat, peak, ridge, shoulder, spur, slope, hollow, footslope, valley, pit) after Jasiewicz and Stepinski (2013): in 8 directions out to a lookup distance, the zenith and nadir angles against a flatness threshold give a ternary pattern, and the counts of higher and lower directions select the class. Defaults are a lookup distance of 1 km and a threshold of 1 degree. The results SHALL be thread-count independent (CON-14).

Verify: A
Status: planned
Source: SPIKE-57, CON-14

#### Scenario: Forms
- **WHEN** the classifier runs on a cone, an inverted cone, a V valley, a ridge and a tilted plane
- **THEN** the apex is a peak, the inverted apex a pit, the valley axis a valley, the ridge axis a ridge, and the plane slope (flat below the threshold)

### Requirement: GEN-092 Drainage descriptors
The descriptors module SHALL fill depressions by a priority flood with an epsilon of 1e-3 m, ties broken by cell index. It SHALL then route D8 (steepest descent, ties by direction order), accumulate the drainage area in topological order, and give the drainage density: the length of the channel cells (area at least a threshold, default 1e5 m2) over the counted area, in km per km2. The results SHALL be thread-count independent (CON-14).

Verify: A
Status: planned
Source: SPIKE-57, SPIKE-40 (drainage density target), SPIKE-47, CON-14

#### Scenario: A tilted V valley with a pit
- **WHEN** the drainage runs on a tilted V valley holding one pit
- **THEN** the pit is filled, the area grows downstream along the axis, and the density equals the analytic channel length over the area within 1 %
