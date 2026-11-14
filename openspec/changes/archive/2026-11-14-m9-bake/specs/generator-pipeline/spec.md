## ADDED Requirements

### Requirement: GEN-093 Relief classes are class weights on a lattice
The generator SHALL describe relief classes as class weights per lattice point. The lattice SHALL have the geometry of the landform lattice (ADR 0080 point 3): on each cube face the points at the face coordinates `2 i / 256 - 1`, read bilinearly, continuous across face edges; it holds class weights, not the landform. Each point SHALL carry one weight per class of a versioned list; the list of version 1 SHALL be plain, rolling plain, hills, mountains, high mountains, plateau, badlands and sand sea, in that order. The weights SHALL be stored as `u8` that sum to 255 per point: the f64 weights are scaled by 255 and rounded down, and the remainder goes one unit at a time to the classes with the largest fractional parts, ties to the earlier class in the list. The class source, the weights and every value derived from them SHALL be CPU fields computed with `libm` and integer hashing, bit-exact for any thread count, with no GPU twin (CON-07, CON-14). Every consumer SHALL read the weights only through this lattice.

Verify: A
Status: planned
Source: ADR 0089 decisions 1, 2 and 10, ADR 0088/0089 accepted 2026-11-01; ADR 0080 point 3 (the lattice geometry); SPIKE-77, SPIKE-80

#### Scenario: Weights sum and stay the same bits
- **WHEN** the default weights are built for a seed on 1 and on 8 threads
- **THEN** every point's `u8` weights sum to 255, both builds give identical bytes, and two points 1e-12 either side of a cube edge read weights that differ by under 1e-9 after interpolation

### Requirement: GEN-094 The plate mask and the uplift fields
The default class source SHALL compute, as pure functions of the seed and the direction:
- **Plates:** 14 centres and Euler-pole rotation vectors (rates 0.3 to 1.0, dimensionless) from a hash of the seed and a draw index; up to 64 draws (draw 0 the seed itself, then derived seeds) are scored by the share of land (8192 points of a Fibonacci sphere) that lies near a convergent margin, `(1 - smoothstep(150, 450 km, d)) smoothstep(0.05, 0.35, convergence)` of its two nearest plates, and the first draw with the highest score is kept, so that the belts stand on the continents (GEN-105); the direction warped by 0.35 times a two-octave fBm of frequency 1.3. A plate's **land share** is the fraction of its area that is land, from 2048 points of a Fibonacci sphere assigned by the warped direction's nearest centre.
- **Pairs:** the pairs (nearest, second), (nearest, third) and (second, third) of the three nearest centres of the warped direction, with angles `a1` to `a4` for the four nearest. Every term below is the largest over the pairs, each pair's term weighted by the product of its two plates' weights, a plate of rank `r` (1 to 3) weighing `smoothstep(0, 0.05 rad, a4 - a_r)` (so a plate has weight 0 where it trades places with the fourth, and two plates that trade places keep their weights), and for (second, third) also by `1 - smoothstep(0, 0.04 rad, a2 - a1)`. Each pair's term is symmetric in its two plates, so the mask is continuous where the nearest plate changes. (The pair's signed distance below is positive on the side of the plate with the lower index, which is what keeps the term continuous where the nearest plate changes.)
- **Margin distance** `d` of a pair: half the angular difference of its two centres' angles, times the body's radius. The preview uses 6,371 km; the bake SHALL read the body's radius from the definition (CON-11).
- **Convergence** of a pair: the difference of the two plates' velocities (rotation vector cross direction) against the unit margin normal (from the first towards the second centre, in the tangent plane), positive where they move towards each other.
- **Distances to the coast and to the coastal line:** the signed `(land depth - 0.10) / |gradient|` (`d_c`, positive inland of the line) and `land depth / |gradient|` for the coast, both of the smooth land depth (the continent stage without its detail octaves and islands, GEN-104: the full value's gradient changes several times over one lattice cell), the gradient by central differences of 0.004 rad along the axes x, y and z with the radial part removed (a first-order estimate, valid near the line, the gradient floored at 1e-9).
- **Core:** the larger of a belt `smoothstep(0.05, 0.35, convergence) max(0, 1 - |u - b| / W_b)` and a coastal belt `a o near smoothstep(0.05, 0.35, convergence) max(0, 1 - |d_c - b| / (0.8 W_b))`, times the strike factor. Here `u` is the signed distance to the margin of the pair (positive on the side of the plate with the lower index, so that it is continuous where the nearest plate changes), `d_c` the signed distance to the line at land depth 0.10, and `b = 120 km` times a two-octave fBm of frequency 3.0 in `[-1, 1]` bends the trace of the belt sideways. `W = 200 + 220 n` km with `n` a two-octave fBm of frequency 2.5 in `[0, 1]`; `W_b = max(125 km, W (0.55 + 0.45 s) (0.9 + 0.2 h))`, so that a belt tapers in width as well as in height towards the ends of its segment, never under 125 km (the coastal belt's half-width 0.8 W_b is at least 100 km). `a = smoothstep(0.38, 0.56)` of a two-octave fBm of frequency 0.9 in `[0, 1]`; `o = c_x (1 - c_y) + c_y (1 - c_x)` is 1 where one plate of the pair is oceanic and the other not, with `c = smoothstep(0.15, 0.40, land share)`; `near = 1 - smoothstep(250, 500 km, d)`. The coastal belt thus lies only where a convergent margin with an oceanic side is within a few hundred km. The strike factor is `s (0.45 + 0.55 h)`, with `s = smoothstep(0.25, 0.55, g)`, `g` a two-octave fBm of frequency 2.8 in `[0, 1]` (gaps between segments where it is low), and `h` a two-octave fBm of frequency 5.0 in `[0, 1]` (the height and width along the strike).
- **Flank:** the same terms at three times `W_b` (bent the same way), also times the strike factor (the mask exposes it as `strike`). **Old margin:** `(1 - smoothstep(-0.05, 0.15, convergence)) max(0, 1 - d / 300 km)`.
- **Back-arc:** for the pairs with the nearest plate: `smoothstep(0.7, 0.95, convergence) (1 - smoothstep(450, 700 km, d)) smoothstep(0, 150 km, d) smoothstep(0, 150 km, d1) smoothstep(0.1, 0.5, inland) smoothstep(-0.08, 0.08, share_nearest - share_other)`, `d1` the distance to the nearest margin: it is 0 at a margin and prefers the plate with the larger land share smoothly.
- **Hilly country:** `smoothstep(0.50, 0.76, c_h) smoothstep(0.1, 0.4, smooth land depth)`, `c_h` a three-octave fBm of frequency 2.2 in `[0, 1]` (hills away from the belts; the smooth depth, because the rough `inland` changes several times over one lattice cell).
- **Plateau field:** `smoothstep(0.45, 0.70, c) smoothstep(0.2, 0.6, inland) (1 - smoothstep(0.2, 0.6, max(flank, back-arc)))`, `c` a three-octave fBm of frequency 1.6.
Core, flank and old margin SHALL be multiplied by the coast gate `smoothstep(50, 300 km, distance to the smooth coast) smoothstep(0, 0.3, smooth land depth) smoothstep(0, 0.2, land depth)`, 0 where the smooth or the full land depth is not positive (no belt in the sea or on land the smooth coast does not hold; no belt stops at the coast in a step), where land depth is the continentalness minus the sea level and `inland` is the bake's `smoothstep(0.02, 0.35, land depth)` (`global.rs` `terrain`). The mask SHALL replace the range zone as the mountain strength. No gate of the mask SHALL switch over less than 100 km. The numbers here are start values (SPIKE-79, SPIKE-80, round 1 of the preview iteration); R7 tunes them. The planet-wide check of the belts is TEST-082.

Verify: A
Status: active
Source: ADR 0089 decision 3, ADR 0088/0089 accepted 2026-11-01; SPIKE-79 (14 plates, warp 0.35), SPIKE-80 (width variety, flanks, back-arc), SPIKE-91 (belt width, segments, rings); planner critiques of the preview (2026-11-01 and 2026-11-02, inbox #39 and #40); numerics review of the preview (the jumps at plate bisectors)

#### Scenario: Pure and continuous
- **WHEN** the mask is evaluated on 1 and 8 threads, at points 1e-12 either side of cube edges and corners, and at points 1e-9 rad either side of a plate bisector
- **THEN** both runs give identical bits, the points either side of a cube edge differ by under 1e-9 in every term, and those either side of a bisector by under 1e-6 in core, flank, old margin, back-arc, plateau field and `e0` in km

### Requirement: GEN-095 The default classes follow rules over the mask
The default class weights SHALL be memberships blended by smoothsteps, then normalised by GEN-093. Here `e_class` is the base elevation of GEN-096 without the hill band (GEN-098), which breaks the loop between the classes and the band, and with the ridge factor `0.5 + 0.5 r` replaced by 0.75, so that the weights do not follow the ridge noise's fine structure (GEN-097's limit on the step of `k`):
- mountains `mt = smoothstep(0, 0.4, max(core, flank))` (the membership rises over the belt's width and covers its flanks, so that the class weights change over the belt and not in one lattice step), of which high mountains `mt smoothstep(1000, 5500 m, e_class)` and mountains the rest;
- hills `(1 - mt) smoothstep(0.1, 0.35, max(flank, old margin, hilly))`;
- plateau `(1 - mt - hills) smoothstep(0.3, 0.6, max(plateau field, back-arc))`, so that the memberships partition 1 before the normalisation;
- the rest `1 - mt - hills - plateau`, rolling plain by `smoothstep(0.45, 0.6)` of a three-octave fBm of frequency 3.0, else plain.
Badlands and sand sea SHALL have weight 0 in the default source; class paint (ADR 0089 decision 8) or a derived source sets them. The thresholds are start values (SPIKE-80); R7 tunes them. The planet-wide check of the class shares is TEST-082.

Verify: A
Status: active
Source: ADR 0089 decision 3, ADR 0088/0089 accepted 2026-11-01; SPIKE-77, SPIKE-80; planner (2026-11-01): the plateau as the difference

#### Scenario: Memberships
- **WHEN** a lattice point has core 1, `e_class` 5500 m, and another has core 0, flank 0, old margin 0 and plateau field 1
- **THEN** the first holds weight 255 in high mountains and the second 255 in plateau

### Requirement: GEN-096 The base elevation takes the belts and the plateaus
`global::sample`'s land elevation SHALL be the coast ramp (`620 m smoothstep(0, 1.2, land depth)`; the default class source's `e0` applies it now and `global::sample` when the bake is wired; it was 1000 m over 0.9, which put the median land at 1,180 m against Earth's 410 m, SPIKE-93) plus:
- the belt term `7800 core (0.5 + 0.5 r)` m, with `r` the bake's squared ridge term (`global.rs` `ridge_at`);
- the back-arc uplift `2000 back-arc` m;
- the old-margin and flank uplift `200 max(old margin, 0.5 flank)` m;
- the plateau uplift `600 plateau field` m;
- the class hill band (GEN-098).
The sum SHALL be clamped to `MAX_ELEVATION_M` (9000 m), which it can reach. Cap bounds SHALL clamp the same way and stay sound where the clamp acts. This replaces the range term `5200 ridge zone inland` and GEN-064's band, and `GLOBAL_VERSION` SHALL become 7 (6 is the continent stage, GEN-104). The constants are start values (SPIKE-78, SPIKE-80); R7 tunes them. The planet-wide check of the hypsometry is TEST-082.

Verify: A
Status: planned
Source: ADR 0089 decision 7, ADR 0088/0089 accepted 2026-11-01; SPIKE-78, SPIKE-80 (land above 0.5 / 1 / 2 / 3 / 4 / 5 km 48 / 27 / 7.6 / 3.3 / 1.8 / 0.75 %, highest 8.0 km)

#### Scenario: Clamp and bounds
- **WHEN** a direction whose terms sum above 9000 m is sampled, and caps around it are bounded
- **THEN** its elevation is 9000 m and lies in every cap's bounds, and no sample of 40,000 directions spread over the sphere exceeds the clamp

### Requirement: GEN-097 Amplitudes per class
Each class SHALL carry the relief driver `k`, the fine-crease floor, the carve factor (a factor on the carve's `K dt`, ADR 0089's `K t`) and the deposition's `kc`, blended by the class weights:
| Class | `k` | Crease floor | Carve factor | `kc` |
|---|---|---|---|---|
| plain | 1 | 0.2 | 1 | 1e5 |
| rolling plain | 1 | 0.35 | 1 | 1e5 |
| hills | 5 | 0 | 1 | 1e5 |
| mountains | 6 | 0 | 1 | 1e5 |
| high mountains | 7.5 | 0 | 1 | 1e5 |
| plateau | 1 | 0 | 1 | 1e5 |
| badlands | 2 | 0 | 1 | 1e5 |
| sand sea | 0.35 | 0 | 0.5 | 1e5 |
- The blended `k` SHALL be tapered to 1 by `1 + smoothstep(0, 400 m, m) (k - 1)` towards the coast, with `m` the ring mean (GEN-099).
- The 99th percentile of the steps of the untapered blended `k` between neighbouring lattice points SHALL be at most 2.0 (owner, 2026-11-02: the limit applies to 99 % of the steps, not to every step); the largest step is reported, not gated. The limit is a parameter of the bake (`max_step`, default 2.0). A class paint item SHALL be feathered over at least `ceil(|delta k| / max_step)` lattice cells, so that it adds no step over `max_step`.
- The crease and detail amplitudes SHALL no longer depend on the landform driver or on the base elevation above 300 m. The erosion field's version SHALL become 3.
On seeds 1 to 5 the default source's p99 is 1.4 to 1.8 and its largest step 5.1 to 5.8 (0.4 % of the steps are over 2.0): a belt's flank changes by 0.2 to 0.3 of its core over one 40 km cell and the classes differ by 4 to 6 in `k`. Earth has 6.7 % of 40 km pairs over 2.0 (largest 5.4, p99 3.3), so the p99 form is the engine's own smoothness rule, not a realism target.
The table and the limit are start values (SPIKE-73, SPIKE-76 to SPIKE-81; `kc` from SPIKE-64); R7 tunes them on the scorecard. The planet-wide check of the step limit is TEST-082.

Verify: A
Status: active
Source: ADR 0089 decisions 5 and 6, ADR 0088 point 1, ADR 0088/0089 accepted 2026-11-01; SPIKE-73, SPIKE-76, SPIKE-77, SPIKE-78, SPIKE-80, SPIKE-81

#### Scenario: Blended amplitudes
- **WHEN** a lattice point holds weights 128 hills and 127 mountains
- **THEN** its untapered `k` is `(128 x 5 + 127 x 6) / 255`, and at a ring mean of 0 m the tapered `k` is 1

### Requirement: GEN-098 The hill band follows the class
The hill band SHALL keep GEN-064's three octaves, weights, warped direction and coast ramp, and SHALL take the amplitude `30 + 145 smoothstep(1, 5, k) + 650 smoothstep(5, 7.5, k)` m from the untapered blended `k` (GEN-097) instead of the range zone's mountain strength. `HILL_BAND_MAX_M` SHALL become 825 m. Where the class is mountains or high mountains the band SHALL be x4.5 as high (the factor is blended by the two classes' weight), its noise a ridged multifractal of five octaves (16, 8, 4, 2 and 1 km, each octave weighted by the previous one's ridge so that the ridges stay connected) in place of the three octaves, blended by the same weight, and centred (0.45 of the unit noise, its mean, is subtracted, so that the larger band does not lift the median elevation); land SHALL stay at least 1 m above the sea level where the base is (the shore of the continent stage is kept). A cap's bounds SHALL carry the band's per-octave noise bounds at the cap: for the hill octaves as before (`HILL_BAND_MAX_M`), and where the mountain weights are not 0 the ridged octaves' bound in `[0, 1]` and the factor 4.5 (the band's maximum is `0.55 * 4.5 * 825` m, about 2.0 km, and its minimum `-0.45 * 4.5 * 825` m before the land floor), all inside the clamp of 9000 m. The curve and the factors are start values (SPIKE-80; SPIKE-101, SPIKE-102 run a: x4.5 ridged centred reaches 0.79 of Everest's 3 km relief on the base); R7 tunes them. `CLASS_SOURCE_VERSION` SHALL become 6.

Verify: A
Status: active
Source: ADR 0089 decision 6, ADR 0088/0089 accepted 2026-11-01; SPIKE-80 (the hill landforms fail criterion 1 without the band), SPIKE-82

#### Scenario: Band by class
- **WHEN** the band is evaluated at the same direction with `k` 1, 5 and 7.5
- **THEN** its amplitude is 30, 175 and 825 m, and its value lies in the bounds of every cap that holds the direction

#### Scenario: The mountain band
- **WHEN** the band is evaluated at 500 or more land points whose mountain and high-mountain weights sum to over 0.99
- **THEN** its value lies in `[-0.45, 0.55]` of `4.5` times the amplitude of GEN-098's curve, and its mean is within 0.05 of that amplitude of 0

### Requirement: GEN-099 The relief channels and the runtime base field
The bake SHALL compute two channels on the coupled lattice (2048 cells per face edge, about 4.9 km), as f32 with min and max per 32 x 32 block:
- the relief driver `k` (GEN-097, tapered);
- the ring mean `m`, the mean of the bake's elevation at the point and at the 8 compass points of a ring of 10 km radius (9 samples, a fixed order; the compass is east and north, east being the Z axis crossed with the direction (the Y axis at a pole), so the ring's orientation is continuous over every face edge and diagonal and a point shared by two faces has the same value on both).
The tapered `k` is `1 + smoothstep(0, 400 m, m) (k - 1)` of the class weights' blended driver (GEN-097). The channels are computed from the direction alone (the bake's elevation and the class weights as inputs), f64 with `libm`, stored as f32, and the same bytes for any thread count. A block's range covers the points of its cells (including the shared row and column of the next block). `RELIEF_CHANNELS_VERSION` 1 (`relief_channels.rs`).
The runtime base field SHALL be `m + k (e - m)` plus `k` times the creases (above the erosion layer's grid only the octaves GEN-109's band keeps, where the layer read calls it), with `e` the bake, each term one read at the point. It is behind a switch (`ErosionField::with_relief`), off by default:
- off: `k` is 1 and `m` is not read, today's field bit for bit, its key and implementation id unchanged;
- on: the implementation id is `cpu-erosion-v1+relief`, and the inputs hash folds the channels' identity (a hash of their content), their resolution and the channels' version; the body's relief scale SHALL be 1 (the channels are of the bake as it is), else the switch returns an error naming the scale;
- the reader (`relief_lattice.rs`) interpolates the stored f32 points in f64, continuous across face edges, takes the channels' cells per face edge from the channels (a coarse planet pass and a full one read through the same code), and returns an error naming the face for a channel with a value that is not finite.

Verify: A
Status: active
Source: ADR 0088 points 1 and 2, ADR 0088/0089 accepted 2026-11-01; m9-bake PR 1 computes `m` from the existing bake; SPIKE-66 (the 10 km ring; a runtime stencil breaks batch reads and bounds)

#### Scenario: One read per term
- **WHEN** the base field is evaluated for a batch of directions
- **THEN** each direction reads `e`, `k` and `m` once at its point, and the batch equals bit for bit the heights evaluated one by one (GEN-020)

### Requirement: GEN-100 Pass 1 carves each cube face on a coarse grid
The bake's first pass SHALL, per cube face:
- sample the base field (GEN-099) once per cell of 814 m (one point sample per cell);
- carve it with implicit stream power with uplift (`U / K` 20, `K dt` 0.05 times the carve factor of GEN-097), 20 steps, not to steady state; the uplift of a cell is tapered by its starting height `h` above the sea level, `smoothstep(0, 60 m, h)` of it (a coastal plain that barely erodes is not lifted by the whole 20 m: owner 2026-11-04, the coast diagnosis);
- breach after each route (the next fill sees the breached surface): from the leaves down, the receiver of a cell that stands higher than 0.1 % of the cell distance below it is lowered to that level, by at most `breach_max_m` for each donor that reaches it (500 m, SPIKE-101: with 50 m the carve fills the basins the hill band leaves and the layer halves the high mountains' relief; 0.62 of Everest's 3 km relief and 0.91 of the Alps' with 500 m), never an outlet (the sea and the face's border cells) and never below the sea level; an option `breach_height_scale_m` (off, 0) scales the limit from 50 m at the sea level to `breach_max_m` over that height of the receiver above it;
- keep the pits (GEN-120): the route runs over the filled copy, but a cell that stands below its receiver is not raised to the filled level, it only gains its uplift (`keep_pits`, on; an incise-only carve ignores it), and the face's own cells' depth below the flood of the carved heights is kept as the face's fill depth;
- keep land above the sea level (plus the flood's epsilon) whatever depth the sea outlets have;
- route with Barnes' improved priority flood (epsilon 1 mm), jittered D8 receivers (the jitter an integer hash of the GLOBAL cell and the direction, so a cell routes alike in every window that holds it) and the order from the receivers, and the drainage area by multiple flow direction (Freeman, exponent 1.1: a cell shares its area among its lower neighbours in proportion to the slope to the power 1.1, highest cells first, ties by the cell index; an outlet passes nothing on) which drives the carve while the receivers stay D8 (a D8 area imprints the grid's eight directions as staircase valleys: SPIKE-98 diagnosis, owner 2026-11-03), re-routing every 5th step; the carve factor multiplies `K dt` and the uplift (`U / K` stays 20); a height or factor that is not finite, a flood without an outlet or with an epsilon that is not positive SHALL be refused;
- not route across cube edges: each face is carved on its own grid extended by a halo of `ceil(cells / 128)` cells (at least 4) beyond each edge over the plane that continues the face, so the face's own border is an ordinary cell; the stored change keeps the halo; a reader SHALL cross-fade the faces within the halo's width of an edge (the face's own weight 1 and a neighbour's `1 - smoothstep(distance to the shared edge / halo)`, normalised; at a corner both neighbours count), so that the change is continuous across the edges and around the corners; a halo of half the face or more SHALL be refused.
It SHALL keep each face's change as a record of the layer store (STRM-065) and checkpoint per face. The constants are start values (SPIKE-62, SPIKE-65, SPIKE-66, SPIKE-70, SPIKE-71).

Verify: A, B
Status: planned
Source: ADR 0088 point 4.1, ADR 0088/0089 accepted 2026-11-01; SPIKE-62, SPIKE-65, SPIKE-66, SPIKE-70, SPIKE-71

#### Scenario: The breach limit
- **WHEN** a basin ringed by a 400 m dam is carved for one step with the default breach and with a limit of 50 m, and with the height scaling at 2000 m
- **THEN** the dam is lowered further with the default than with 50 m, the scaled limit gives a dam between the two, and `breach_height_scale_m` is part of the layer key

#### Scenario: The uplift is tapered
- **WHEN** a plain at 3 m and a plateau at 500 m, each ringed by outlets, are carved with the taper of 40 m and without it
- **THEN** the plain is lifted by over 5 m without the taper and by under 1.5 m with it, the plateau's bits are the same with and without it, and a carve without uplift is the same with either setting

#### Scenario: The change over the shallow sea
- **WHEN** pass 1 runs on a face of a low plain beside a shallow sea, and the change is read at the cell centres and at 4 x 4 points a cell (as pass 2 reads it) wherever the base is at or below the sea level, with the taper of 40 m and without it
- **THEN** without the taper the uplift of the shore bleeds more than 2 m onto the sea points, with it the change over the sea is at most 0.01 m

#### Scenario: A small face
- **WHEN** pass 1 runs on a synthetic face of 64 x 64 cells uninterrupted on 1 thread (the reference), on 8 threads, and on 1 thread killed and resumed
- **THEN** the second and third runs equal the reference byte for byte

#### Scenario: The area is shared
- **WHEN** the routing runs on a plane falling along a diagonal, with two jitter streams
- **THEN** the receivers differ and the areas are the same bits; the area is mirror-symmetric about the diagonal, a cell off the diagonal holds more than its own cell, and the area leaving the outlets is the area of the grid

#### Scenario: One face timed (B)
- **WHEN** pass 1 runs on one face of the store at 814 m
- **THEN** its time and peak memory are reported, not bounded (SPIKE-71's about 3 min and 5.5 GB are superseded: SPIKE-98 measured about 17 to 23 minutes a face at 10 threads with the area by multiple flow direction)

### Requirement: GEN-101 Pass 2 works per tile of the layer's grid
The bake's second pass SHALL work per tile of the layer grid: 98,304 cells per face edge (101.8 m), tiles of 3,072 x 3,072 core cells with a 128-cell halo (SPIKE-61's tiles; SPIKE-98 measured the same core as at 256 except near the edges). Per tile it SHALL take:
- the base field, evaluated on every 4th cell of the window and interpolated between by Catmull-Rom kept within the range of the four nodes around the point (the base is 86 % of a tile's cost; the stored change is taken against this interpolated window, so the interpolation error cancels in it: design.md), plus the face carve's change (read by Catmull-Rom (C1) in a face, kept within the range of the four cells around the point (a Catmull-Rom spline overshoots by up to 7.4 % of a step beside a sharp feature, which would read a one-cell channel as a levee and could put a coastal plain below the sea), since a bilinear read leaves a crease along every cell line of the pass-1 grid that the carve and the viewer would show; cross-faded within the halo of a cube edge: GEN-100);
- a local carve of catchments up to 96 cells (1 km^2 at 101.8 m) that only incises: no fill, no breach, no uplift (the routing of GEN-100, the area by multiple flow direction);
- thermal at a critical slope of 33 degrees, perturbed by up to 25 % (an integer hash of the unordered pair of global cell ids), moving 1/8 of the excess per pass (a share over 1/8 SHALL be refused: a cell would overshoot), at most 300 passes, on active cells (the cells that moved and their neighbours: bit for bit the run over every cell); a pair over the critical difference by under 1 mm does not move (the excess would never reach 0), and a cell whose height rounding leaves unchanged wakes no neighbour; the sweep keeps a cell at or below the lowest final height of its donors; land above the sea level off the grid's border gives, any cell receives;
- one deposition sweep (on its own route, whose area is by multiple flow direction too) with `kc` (1e5 uniform while every class of GEN-097 has 1e5; per class when they differ) and `zeta` 0.5 on a route of its own on the carved and thermally settled window (the load of a cell is its donors' plus its eroded column; it fills the depression first (at most to the receiver's carved height plus half the filled drop: a closed bowl's route does not become a pillar of the bowl's fill), then `zeta` of the excess over the capacity `kc sqrt(A) S` settles, at most to the filled level plus half the drop to the receiver; what reaches an outlet leaves; eroded = deposited + exported).
- a pit spreading after the sweep (`pit_min_m` 0.5 m, at most `pit_passes` 32): a land cell whose 8 neighbours are inside the grid's border, none at or below the sea level, and all higher than it by more than `pit_min_m` rises by 0.8 of its depth below the lowest neighbour while its 8 neighbours each fall by an eighth of that rise, so the sum of the heights is unchanged; the pits are taken in cell order, the pass repeats until none is deeper than `pit_min_m` (the neighbours' fall can make a smaller one), and the border cells are never lowered; serial and deterministic.
It SHALL store the core's change against the base field (STRM-065) and checkpoint per tile, with per tile the cells that still gave in the last thermal pass (the unsettled slopes at the pass cap; the pass cap is the only bound on thermal: no wall-clock budget, which would make the result depend on the machine, CON-14). The tile grid, the halo and the tiles of a run (every land tile, or a region list) are parameters of the run; the local carve runs 10 steps without uplift, breach or fill, with the drainage area capped at 96 cells (1 km^2 at 101.8 m; the cap follows the cell size, so a coarse planet pass keeps the same number of cells), and a window's cells follow the global cell of the face's grid in the jitter and the thermal's pair hash. Every kernel SHALL be a Jacobi gather or a fixed topological sweep in f64 with `libm`. The constants are start values (SPIKE-61, SPIKE-64, SPIKE-70, SPIKE-72). The seam check is TEST-084.

Verify: A, B
Status: active
Source: ADR 0088 points 4.2 and 6, ADR 0088/0089 accepted 2026-11-01; SPIKE-61, SPIKE-64, SPIKE-70, SPIKE-71, SPIKE-72, SPIKE-73

#### Scenario: Thermal on active cells
- **WHEN** thermal runs on one synthetic tile of 3,584 cells a side (the kernel defaults of 100 passes of 1/16, 4 threads; the production setting is set in `Pass2Params::standard`), an upland and a steeper one
- **THEN** it keeps the mass, gives the bytes of the run over every cell, evaluates at most 5 % (upland) or 25 % (steeper) of the cells of the full run, and takes 5 s on the reference machine for the upland (the test bounds the time by ADR 0087's loaded margin; the cost grows steeply with relief: see the design)

#### Scenario: The base is sampled sparsely
- **WHEN** the base of a window is sampled every 4 cells and interpolated, on 1 and on 8 threads
- **THEN** the bytes are equal, the value at a node is the field's, and between the nodes it is within 5 m of a field of amplitude 550 m and wavelength over 20 cells and always within the range of the four nodes around the point (so a cliff does not put a coastal plain below the sea)

#### Scenario: The pass-1 change is read smoothly
- **WHEN** the pass-1 change is read at the centre of a cell and just before and after it, along each axis
- **THEN** the value at the centre is the cell's, and the slope before equals the slope after to 3 % (a bilinear read does not)

#### Scenario: The read stays in range
- **WHEN** the pass-1 change holds one cell of -100 m among zeros and is read at points around it
- **THEN** every read lies between -100 and 0 m

#### Scenario: Pits are spread
- **WHEN** a land with pits of 3 to 70 m (two adjacent, one beside the border, one beside the sea) is spread with a threshold of 0.5 m, and a tile of the synthetic planet is run
- **THEN** no pit over 0.5 m is left, the sum of the heights is unchanged to 1e-9, the border and the sea cells keep their bits, the same input gives the same bits, a pass count of 0 moves nothing, and no one-cell pit over 0.5 m is left in the tile's core

#### Scenario: A bowl's fill is not a pillar
- **WHEN** the deposition sweep runs on a closed bowl of 80 m in a plateau
- **THEN** what was eroded is deposited or exported to 1e-9 and no cell of the bowl rises by 60 m or more

#### Scenario: Incise only
- **WHEN** pass 2's local carve runs on a synthetic tile with a closed basin
- **THEN** no cell ends higher than its input, and the basin is not filled

### Requirement: GEN-110 Pass 1 keeps the drainage area of its final route
Pass 1 SHALL return, with the face's change, the drainage area of the face's own cells from the last route of its carve (by multiple flow direction, GEN-100) as one byte a cell: `round(8 log2(area / cell area))`, so 0 is a cell's own area, one step is a ninth more area, and 255 is 2^31.9 cells; a larger area is stored as 255. The face's change SHALL be the same bytes with and without the area, and the codes the same bytes on any thread count (CON-14). The area serves the viewer's ground materials and detail; it is not an input of pass 2.

Verify: A
Status: active
Source: owner 2026-11-04 (planner inbox #58.3, M9 plan I5a(c)); GEN-100

#### Scenario: The area codes
- **WHEN** pass 1 runs on a synthetic face on 1 and on 2 threads, with and without the area
- **THEN** the change is equal in both, the codes are equal on both thread counts, a cell with nothing above it has code 0, and some cell has at least 8 (an area of two cells)

### Requirement: GEN-111 Pass 2 keeps the deposited depth of its core
Pass 2 SHALL return, with the core's change, one byte plane over the core: the depth deposited as a square-root code, `round(12 sqrt(m))` of the depth in metres (0 to 451.6 m; the step is 0.10 m at 0.3 m, 0.12 m at 0.5 m, 0.17 m at 1 m and 0.53 m at 10 m, so the material thresholds of 0.3, 0.5 and 1 m are resolved; from 449.8 m on it is stored as 255, which decodes to 451.56 m; owner 2026-11-05, after the linear 1 m form of 2026-11-04):
- deposited is the deposition sweep's rise plus thermal's gain, `max(0, z_final - z_thermal) + max(0, z_thermal - ref)`, `ref` the base plus the pass-1 change where it is positive (a raise by uplift or a filled pit is not a deposit);
- the depth removed is not stored: a reader derives it as `max(0, deposited - change)` with the layer's change at the cell (`removed_depth_m`); it is exact where pass 1 did not raise the cell, short by the raise where it did (15 % of mountain land and 35 % of hills in seed 14's samples), and clipped where the deposited depth is at the code's end (451.6 m); on a cell that only gained material the derived removal is at most half a step of the code plus half a step of the change (0.21 m at 1 m of deposit; the change is stored in steps of 0.25 m), so a rock or scree mask must threshold above that, not at `> 0`;
- the plane SHALL be the same bytes on any thread count.
It serves the viewer's ground materials (soil and scree where deposited, rock where the change is deep and little is deposited).

Verify: A
Status: active
Source: owner 2026-11-04 (planner inbox #58.3, M9 plan I5a(c); the one-plane, 1 m form chosen on the sizes); GEN-101

#### Scenario: The deposited depth and what is removed
- **WHEN** one synthetic tile is run with a face change of -30 m, of +10 m and with none
- **THEN** the plane holds deposits in places and none elsewhere, the codes are 0 for 0 m, 12 for 1 m, 255 from 449.8 m (decoding to 451.56 m), strictly increasing with a step of at most 0.11 m at 0.3 m, 0.12 m at 0.5 m, 0.18 m at 1 m and 0.56 m at 10 m, the round trip is within half a step in the square root, and the depth removed that is derived from a cell's deposited depth and change is never negative and is at most the pass-1 raise (zero where pass 1 did not raise) away from deposited minus change, within the code's step

### Requirement: GEN-102 Cap bounds read the layer's pyramid
The field's height bounds over a cap SHALL take the layer's part from the layer's min/max pyramid (STRM-066):
- the nodes the cap reaches, widened by 4 cells (2 for the stencil, 2 for the cross-fade at a cube edge), on every face it touches; a node wholly inside is taken whole;
- below the 32-cell blocks only for a cap of at most 8 cells' radius, down to nodes of 2 cells;
- a cap that reaches more than 36 tiles takes the layer's global range;
- the layer adds the product of that range and the coast fade's interval over the cap's base elevation (GEN-113).
With the relief channels on the bounds SHALL also take:
- the channels' range `[k_min, k_max, m_min, m_max]` over the cap: the union of the ranges of the blocks the cap touches, from a pyramid of the block ranges (a node wholly inside the cap is taken whole), and for a block the cap only touches the min and max of the lattice points of the nodes of 2 cells it touches;
- the base `m + k (e - m) + k detail` over the intervals of `e`, `m`, `k` and the detail (products of intervals, a sound bound that ignores the correlation of `e` and `m`);
- the planet-wide bound `max|1 - k| max|m| + k_max (max|e| + max|detail|)`, from `(1 - k) m + k (e + detail)`.
Because each pyramid level holds the range of the levels below and of the nodes, the bounds of nested caps SHALL nest (LOD-012). The plan check at production parameters is TEST-083.

Verify: A
Status: active
Source: ADR 0088 points 3 and 8, ADR 0088/0089 accepted 2026-11-01; SPIKE-82, SPIKE-83 (the steep classes plan to level 18 with no capped tile, 2.5-4 us per lookup)

#### Scenario: Sound and nested
- **WHEN** 20,000 random caps of radius 1e-6 to 0.01 rad over a synthetic layer are bounded, rims and face edges included, each with a nested cap inside it
- **THEN** every sampled height lies in its cap's bounds, and each inner cap's bounds lie within its outer cap's (within LOD-012's slack)

### Requirement: GEN-112 The runtime read of the erosion layer
`ErosionField::with_layer(layer, sea_level)` MAY add the stored layer (STRM-065) to a relief field; without the call the field is today's, bit for bit:
- it switches the crease band of the bake on (GEN-109) and needs no relief channels (without them the base is the lattice read at `k` 1, `m` unread);
- the layer's change is the stored change with the pit depth added back (GEN-120, until lake water and a pit-tolerance rule exist), and the pyramid (GEN-102) and the range of the manifest hold that sum;
- the height is the base (the relief base with the channels) plus the layer's change times the coast fade (GEN-113), not scaled by `k`;
- the change is the monotone C1 read of the cell centres (GEN-188), within the four cells around the point, reading the neighbouring tiles' cells, clamped at a face edge and cross-faded over 2 cells with the neighbouring face's read; a tile the layer lacks is 0;
- the fine detail is pinned to the cell centres (GEN-187).

Verify: A
Status: active
Source: ADR 0088 point 5 as amended by ADR 0100 (the runtime read is C1, with the coast fade and the cube-edge cross-fade; owner 2026-11-04), ADR 0098, ADR 0099; m9-bake PR 6

#### Scenario: Off and zeros
- **WHEN** the call is not made, or the layer holds only zeros
- **THEN** without the call every height equals the relief field's to the bit; with zeros it is the band's field with the pinned detail (GEN-187), on both bases

#### Scenario: The read
- **WHEN** the change is read at cell centres, at random points, along cell lines and across tile seams, and a tile of the layer is absent
- **THEN** a centre reads its stored value, every read lies within its four cells' range, the one-sided slopes at a knot agree, a cell of an absent tile counts as 0, the stencil reads the neighbouring tile's cells, and a batch of heights equals the single reads' bits

#### Scenario: The cube edge
- **WHEN** a layer that is a smooth function of the direction is read within 1e-9 rad either side of every cube edge and around every corner
- **THEN** the reads differ by under a millimetre, where one face's clamped read alone steps by about a cell's change

### Requirement: GEN-113 The coast fade of the layer
The layer's change SHALL be multiplied by a smoothstep fade of the bake's elevation `e`: 0 at or below the sea level, 1 from 3 m above it. The sea level is the one `with_layer` is given (the definition's). The fade follows `e`, not the relief base `m + k (e - m)`, and the bounds take its interval over the cap's base elevation.

Verify: A
Status: active
Source: Owner 2026-11-04 (the coast remedy, I5c), ADR 0100; m9-bake PR 6

#### Scenario: Coast and order
- **WHEN** heights are read over land, over the first 3 m above the sea level and over the sea
- **THEN** land adds the change, a point at elevation `e` inside the first 3 m adds the fade's share of it, the sea adds none, and the change is not scaled by `k`

### Requirement: GEN-114 The keys of a layered field
A field with the layer on SHALL fold the layer's identity, the sea level, the fade's width and the read's version into its inputs hash, and its implementation id SHALL gain `+layer` over the band's. The field's version SHALL NOT change. `with_layer` SHALL be refused after the crease band, and the relief channels SHALL be refused after the layer; without the channels the id is `cpu-erosion-v1+band+layer` (v2 over the C1 base).

Verify: A
Status: active
Source: ADR 0098 consequence (the id and the hash change when the band is on), ADR 0100; m9-bake PR 6

#### Scenario: Keys
- **WHEN** the layer, its sea level or the call order changes
- **THEN** the inputs hash differs, the implementation id gains `+layer`, the version is unchanged, the ids differ with and without the channels, and the refusals name the problem

## REMOVED Requirements

### Requirement: GEN-061 The fine creases follow the landform
**Reason**: ADR 0089 decision 6: the landform-keyed crease factor is removed; the amplitudes follow the class (GEN-097).
**Migration**: m9-bake PR 1, with GEN-097; the erosion field's version becomes 3.

### Requirement: GEN-062 The landform is read from a fixed lattice and bounded over a cap
**Reason**: ADR 0089 decision 1: the landform is superseded as the weight; its lattice geometry carries the class weights (GEN-093).
**Migration**: m9-bake PR 1, with GEN-093.

### Requirement: GEN-064 The hill band and the landform driver of the bake
**Reason**: ADR 0089 decisions 3, 6 and 7: the band follows the class (GEN-098) and the base elevation takes the belts and plateaus (GEN-096).
**Migration**: m9-bake PR 1, with GEN-096 and GEN-098; `GLOBAL_VERSION` becomes 7 (6 is the continent stage of `m9-generator-round-2`, which lands first) and its pinned known answers are re-made.
