## ADDED Requirements

### Requirement: GEN-089 Local terrain descriptors
`planet_generators::descriptors` SHALL compute, on a height raster of cell size `h` with marked no-data cells (NaN), the slope in degrees (`atan` of the central-difference gradient) and the profile and contour curvature in 1/m from the Evans-Young quadric fit over the 3x3 window. Profile curvature is taken along the gradient and contour curvature across it, both positive where the surface curves upward (a bowl). The profile is 0 where the gradient is under 1e-6 and the contour is uncounted where it is under 1e-3 (it would only report noise). A cell whose window touches the edge or a no-data cell SHALL be uncounted. The computation SHALL use f64 and `libm` and give the same bytes at any thread count (CON-14).

Verify: A
Status: active
Source: SPIKE-57 (scorecard, descriptors survey CGF 44(2) 2025), SPIKE-55, CON-14

#### Scenario: Definition
- **WHEN** the window is `z1 z2 z3 / z4 z5 z6 / z7 z8 z9` (row `j - 1` first, `i` along x, `j` along y)
- **THEN** the gradient is `p = (z3+z6+z9-z1-z4-z7)/6h` and `q = (z7+z8+z9-z1-z2-z3)/6h`, the slope uses `(z6-z4)/2h` and `(z8-z2)/2h`, and with `r = (z1+z3+z4+z6+z7+z9-2(z2+z5+z8))/3h^2`, `t = (z1+z2+z3+z7+z8+z9-2(z4+z5+z6))/3h^2` and `s = (z9+z1-z3-z7)/4h^2` the profile curvature is `(p^2 r + 2pq s + q^2 t) / ((p^2+q^2)(1+p^2+q^2)^(3/2))` and the contour curvature `(q^2 r - 2pq s + p^2 t) / (p^2+q^2)^(3/2)`

#### Scenario: Plane and quadrics
- **WHEN** the kernels run on a plane rising 10 degrees east, and on `z = a (x^2 + y^2)` and `z = a x^2` sampled at `(x0, 0)`
- **THEN** the slope is 10 degrees at every counted cell within 1e-9, the profile curvature is `2a / (1 + 4 a^2 x0^2)^(3/2)` on both within 1 %, and the contour curvature is `1 / x0` on the paraboloid and 0 on the trough, also off the axis at `(x0, y0)` with `1/rho` and `rho^2 = x0^2 + y0^2` within 1e-9

#### Scenario: Flat ground
- **WHEN** the kernels run on a plane with 1e-4 m of noise at 30 m cells
- **THEN** every contour curvature is uncounted and the profile curvature stays bounded

#### Scenario: Threads
- **WHEN** the kernels run on the same raster with 1 and 8 threads
- **THEN** the outputs are byte-identical

### Requirement: GEN-090 Relief and position descriptors
The descriptors module SHALL compute the local relief (max minus min) over non-overlapping square blocks of a given side, and the topographic position index (TPI): the height minus the mean over the square window of side `2r+1` cells, from summed-area tables accumulated in f64 in row order. A window that touches the edge or a no-data cell SHALL be uncounted. The results SHALL be thread-count independent (CON-14).

Verify: A
Status: active
Source: SPIKE-57, SPIKE-55 (relief by blocks), CON-14

#### Scenario: Edges
- **WHEN** blocks and windows meet the raster edge or a no-data cell
- **THEN** blocks start at the origin and a partial block at the east or south edge is dropped, a block holding a no-data cell has relief NaN, and the TPI is counted only at data cells at least `r` from the edge whose window holds no no-data cell (a second summed-area table counts the data cells; both tables are built on one thread by row prefix sums)

#### Scenario: Step and wave
- **WHEN** a raster holds a 100 m step through one block and a long sine wave
- **THEN** that block's relief is 100 m and a flat block's is 0, the TPI is positive on the crests and negative in the troughs, and the summed-area mean equals the direct mean within 1e-9

### Requirement: GEN-091 Geomorphons
The descriptors module SHALL classify each counted cell into one of ten landform classes (flat, peak, ridge, shoulder, spur, slope, hollow, footslope, valley, pit) after Jasiewicz and Stepinski (2013): in 8 directions out to a lookup distance, the zenith and nadir angles against a flatness threshold give a ternary pattern, and the counts of lower and higher directions select the class by the table of the reference implementation. Defaults are a lookup distance of 1 km (33 cells at 30 m) and a threshold of 1 degree. A cell closer than the lookup distance to the edge, or whose lines hold a no-data cell, SHALL be uncounted. The results SHALL be thread-count independent (CON-14).

Verify: A
Status: active
Source: SPIKE-57, CON-14

#### Scenario: The pattern
- **WHEN** a direction has the highest elevation angle (zenith) and the lowest (nadir) over the lookup cells, diagonal steps being `h sqrt(2)` long
- **THEN** after the rule `compare_multi` of GRASS `r.geomorphon` (`pattern.c`, mode anglev2), the direction is level (0) when neither angle's size exceeds the threshold, higher (+) when only the zenith's does, lower (-) when only the nadir's does, and when both do the larger size wins and an exact tie is higher; a 0.7 degree plane at a 1 degree threshold is level in every direction

#### Scenario: The class table
- **WHEN** `m` directions are lower and `p` are higher (`m + p <= 8`, 45 pairs)
- **THEN** the class is the entry `forms[m][p]` of `determine_form` in GRASS `r.geomorphon` (`geom.c`), listed by `m` (`p` from 0): 0: FL FL FL FS FS VL VL VL PT; 1: FL FL FS FS FS VL VL VL; 2: FL SH SL SL HL HL VL; 3: SH SH SL SL SL HL; 4: SH SH SP SL SL; 5: RI RI SP SP; 6: RI RI RI; 7: RI RI; 8: PK (FL flat, PK peak, RI ridge, SH shoulder, SP spur, SL slope, HL hollow, FS footslope, VL valley, PT pit)

#### Scenario: Forms
- **WHEN** the classifier runs on a cone, an inverted cone, a V valley, a ridge and a tilted plane
- **THEN** the apex is a peak, the inverted apex a pit, the valley axis a valley, the ridge axis a ridge, and the plane slope (flat below the threshold)

### Requirement: GEN-092 Drainage descriptors
The descriptors module SHALL fill depressions by a priority flood with an epsilon of 1e-3 m, ties broken by cell index. It SHALL then route D8 (steepest descent, ties by direction order), accumulate the drainage area in topological order, and give the drainage density: the length of the channel cells (area at least a threshold, default 1e5 m2) over the counted area, in km per km2. The results SHALL be thread-count independent (CON-14).

Verify: A
Status: active
Source: SPIKE-57, SPIKE-40 (drainage density target), SPIKE-47, CON-14

#### Scenario: Definition
- **WHEN** a raster is filled and routed
- **THEN** a filled cell is `max(z, filled parent + 1e-3)`, the raster edge and the cells next to a no-data cell are the outlets, D8 compares the fall over `h` or `h sqrt(2)` on the filled surface and keeps the first of equal falls in the order N, NE, E, SE, S, SW, W, NW (`j` grows southward), a cell with no lower neighbour has no receiver, the area (own cell included) accumulates in reverse flood order on one thread (topological, as a receiver is strictly lower), a channel cell adds `h` (`h sqrt(2)` for a diagonal receiver) to the length, an outlet adds none, and the counted area is that of all data cells

#### Scenario: A tilted V valley with a pit
- **WHEN** the drainage runs on a tilted V valley holding one pit
- **THEN** the pit is filled, the area grows downstream along the axis by one cell row each step, and the density equals the analytic channel length over the area within 1 %
