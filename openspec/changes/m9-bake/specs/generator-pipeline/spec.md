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
- **Plates:** 14 centres and Euler-pole rotation vectors (rates 0.3 to 1.0, dimensionless) from the seed's hash; the direction warped by 0.35 times a two-octave fBm of frequency 1.3.
- **Margin distance** `d`: half the angular difference between the warped direction's nearest and second-nearest centre, times the body's radius.
- **Convergence:** the difference of the two plates' velocities (rotation vector cross direction) against the unit margin normal (from the nearest towards the second-nearest centre, in the tangent plane), positive where the nearest plate moves towards the other.
- **Core:** the larger of `smoothstep(0.05, 0.35, convergence) exp(-(d / w)^2)` and a coastal arc `a exp(-(d_c / (0.8 w))^2)`. Here `w = 90 (0.6 + 0.9 n)` km with `n` a two-octave fBm of frequency 2.5 in `[0, 1]`, `d_c` the distance to the line at land depth 0.10, and `a = smoothstep(0.40, 0.52)` of a two-octave fBm of frequency 0.9.
- **Flank:** the same two terms at three times their half-widths. **Old margin:** `(1 - smoothstep(-0.05, 0.15, convergence)) exp(-(d / 150 km)^2)`.
- **Back-arc:** `smoothstep(0.7, 0.95, convergence) (1 - smoothstep(450, 700 km, d)) smoothstep(0.1, 0.5, inland)` on the plate with the larger land share of the two, else 0.
- **Plateau field:** `smoothstep(0.50, 0.62, c) smoothstep(0.2, 0.6, inland) (1 - smoothstep(0.2, 0.6, max(flank, back-arc)))`, `c` a three-octave fBm of frequency 1.6.
Core, flank and old margin SHALL be multiplied by `smoothstep(0, 0.04, land depth)`, where land depth is the continentalness minus the sea level and `inland` is the bake's `smoothstep(0.02, 0.35, land depth)` (`global.rs` `terrain`). The mask SHALL replace the range zone as the mountain strength. The numbers here are start values (SPIKE-79, SPIKE-80); R7 tunes them. The planet-wide check of the belts is TEST-082.

Verify: A
Status: planned
Source: ADR 0089 decision 3, ADR 0088/0089 accepted 2026-11-01; SPIKE-79 (14 plates, warp 0.35), SPIKE-80 (width variety, flanks, back-arc)

#### Scenario: Pure and continuous
- **WHEN** the mask is evaluated on 1 and 8 threads, and at two points 1e-12 either side of a cube edge
- **THEN** both runs give identical bits, and the two points differ by under 1e-9 in every term

### Requirement: GEN-095 The default classes follow rules over the mask
The default class weights SHALL be memberships blended by smoothsteps, then normalised by GEN-093. Here `e0` is the base elevation of GEN-096 without the hill band (GEN-098), which breaks the loop between the classes and the band:
- mountains `mt = smoothstep(0.25, 0.55, core)`, of which high mountains `mt smoothstep(2000, 3000 m, e0)` and mountains the rest;
- hills `(1 - mt) smoothstep(0.2, 0.45, max(flank, old margin))`;
- plateau `(1 - mt)(1 - hills) smoothstep(0.3, 0.6, max(plateau field, back-arc))`;
- the rest rolling plain by `smoothstep(0.45, 0.6)` of a three-octave fBm of frequency 3.0, else plain.
Badlands and sand sea SHALL have weight 0 in the default source; class paint (ADR 0089 decision 8) or a derived source sets them. The thresholds are start values (SPIKE-80); R7 tunes them. The planet-wide check of the class shares is TEST-082.

Verify: A
Status: planned
Source: ADR 0089 decision 3, ADR 0088/0089 accepted 2026-11-01; SPIKE-77, SPIKE-80

#### Scenario: Memberships
- **WHEN** a lattice point has core 1, `e0` 3500 m, and another has core 0, flank 0, old margin 0 and plateau field 1
- **THEN** the first holds weight 255 in high mountains and the second 255 in plateau

### Requirement: GEN-096 The base elevation takes the belts and the plateaus
`global::sample`'s land elevation SHALL be the coast ramp (up to 1000 m, unchanged) plus:
- the belt term `5500 core (0.5 + 0.5 r)` m, with `r` the bake's squared ridge term (`global.rs` `ridge_at`);
- the back-arc uplift `2000 back-arc` m;
- the old-margin and flank uplift `200 max(old margin, 0.5 flank)` m;
- the plateau uplift `1200 plateau field` m;
- the class hill band (GEN-098).
The sum SHALL be clamped to `MAX_ELEVATION_M` (9000 m), which it can reach. Cap bounds SHALL clamp the same way and stay sound where the clamp acts. This replaces the range term `5200 ridge zone inland` and GEN-064's band, and `GLOBAL_VERSION` SHALL become 6. The constants are start values (SPIKE-78, SPIKE-80); R7 tunes them. The planet-wide check of the hypsometry is TEST-082.

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
- The untapered blended `k` SHALL change by at most 2.0 between neighbouring lattice points. The default source SHALL meet this, and a class paint item SHALL be feathered over at least `ceil(|delta k| / 2)` lattice cells so that it does.
- The crease and detail amplitudes SHALL no longer depend on the landform driver or on the base elevation above 300 m. The erosion field's version SHALL become 3.
The table and the limit are start values (SPIKE-73, SPIKE-76 to SPIKE-81; `kc` from SPIKE-64); R7 tunes them on the scorecard. The planet-wide check of the step limit is TEST-082.

Verify: A
Status: planned
Source: ADR 0089 decisions 5 and 6, ADR 0088 point 1, ADR 0088/0089 accepted 2026-11-01; SPIKE-73, SPIKE-76, SPIKE-77, SPIKE-78, SPIKE-80, SPIKE-81

#### Scenario: Blended amplitudes
- **WHEN** a lattice point holds weights 128 hills and 127 mountains
- **THEN** its untapered `k` is `(128 x 5 + 127 x 6) / 255`, and at a ring mean of 0 m the tapered `k` is 1

### Requirement: GEN-098 The hill band follows the class
The hill band SHALL keep GEN-064's three octaves, weights, warped direction and coast ramp, and SHALL take the amplitude `30 + 145 smoothstep(1, 5, k) + 650 smoothstep(5, 7.5, k)` m from the untapered blended `k` (GEN-097) instead of the range zone's mountain strength. `HILL_BAND_MAX_M` SHALL become 825 m. A cap's bounds SHALL carry the band's per-octave noise bounds at the cap. The curve is a start value (SPIKE-80); R7 tunes it.

Verify: A
Status: planned
Source: ADR 0089 decision 6, ADR 0088/0089 accepted 2026-11-01; SPIKE-80 (the hill landforms fail criterion 1 without the band), SPIKE-82

#### Scenario: Band by class
- **WHEN** the band is evaluated at the same direction with `k` 1, 5 and 7.5
- **THEN** its amplitude is 30, 175 and 825 m, and its value lies in the bounds of every cap that holds the direction

### Requirement: GEN-099 The relief channels and the runtime base field
The bake SHALL compute two channels on the coupled lattice (2048 cells per face edge, about 4.9 km), as f32 with min and max per 32 x 32 block:
- the relief driver `k` (GEN-097, tapered);
- the ring mean `m`, the mean of the bake's elevation over a ring of 10 km radius.
The runtime base field SHALL be `m + k (e - m)` plus `k` times the creases, with `e` the bake, each term one read at the point.

Verify: A
Status: planned
Source: ADR 0088 points 1 and 2, ADR 0088/0089 accepted 2026-11-01; m9-bake PR 1 computes `m` from the existing bake; SPIKE-66 (the 10 km ring; a runtime stencil breaks batch reads and bounds)

#### Scenario: One read per term
- **WHEN** the base field is evaluated for a batch of directions
- **THEN** each direction reads `e`, `k` and `m` once at its point, and the batch equals bit for bit the heights evaluated one by one (GEN-020)

### Requirement: GEN-100 Pass 1 carves each cube face on a coarse grid
The bake's first pass SHALL, per cube face:
- sample the base field (GEN-099) once per cell of 814 m (one point sample per cell);
- carve it with implicit stream power with uplift (`U / K` 20, `K dt` 0.05 times the carve factor of GEN-097), 20 steps, not to steady state;
- breach before the fill (at most 0.1 % of the cells, at most 50 m deep);
- route with Barnes' improved priority flood, jittered D8 receivers and the order from the receivers, re-routing every 5th step;
- not route across cube edges.
It SHALL keep each face's change as a record of the layer store (STRM-065) and checkpoint per face. The constants are start values (SPIKE-62, SPIKE-65, SPIKE-66, SPIKE-70, SPIKE-71).

Verify: A, B
Status: planned
Source: ADR 0088 point 4.1, ADR 0088/0089 accepted 2026-11-01; SPIKE-62, SPIKE-65, SPIKE-66, SPIKE-70, SPIKE-71

#### Scenario: A small face
- **WHEN** pass 1 runs on a synthetic face of 64 x 64 cells uninterrupted on 1 thread (the reference), on 8 threads, and on 1 thread killed and resumed
- **THEN** the second and third runs equal the reference byte for byte

#### Scenario: One face timed (B)
- **WHEN** pass 1 runs on one face of the store at 814 m
- **THEN** its time and peak memory are reported against SPIKE-71's about 3 min and 5.5 GB

### Requirement: GEN-101 Pass 2 works per tile of the layer's grid
The bake's second pass SHALL work per tile of the layer grid: 98,304 cells per face edge (101.8 m), tiles of 3,072 x 3,072 core cells with a 256-cell halo (SPIKE-61's tiles). Per tile it SHALL take:
- the base field plus the face carve's change, bilinear;
- a local carve of catchments up to 1 km^2 that only incises: no fill, no breach, no uplift;
- thermal at a critical slope of 33 degrees, perturbed by up to 25 %, moving 1/16 of the excess per pass, on active cells;
- one deposition sweep with `kc` from GEN-097 and `zeta` 0.5 on the last route.
It SHALL store the core's change against the base field and checkpoint per tile. Every kernel SHALL be a Jacobi gather or a fixed topological sweep in f64 with `libm`. The constants are start values (SPIKE-61, SPIKE-64, SPIKE-70, SPIKE-72). The seam check is TEST-084.

Verify: A
Status: planned
Source: ADR 0088 points 4.2 and 6, ADR 0088/0089 accepted 2026-11-01; SPIKE-61, SPIKE-64, SPIKE-70, SPIKE-71, SPIKE-72, SPIKE-73

#### Scenario: Incise only
- **WHEN** pass 2's local carve runs on a synthetic tile with a closed basin
- **THEN** no cell ends higher than its input, and the basin keeps its pour point

### Requirement: GEN-102 Cap bounds read the layer's pyramid
The field's height bounds over a cap SHALL take the layer's part from the min/max pyramid (STRM-066):
- the cap's face box is twice its angular radius in face units;
- it is read at the finest pyramid level whose blocks are at least half the box, or from the layer's nodes for a box under 16 cells;
- the runtime detail below the layer's cells adds its analytic per-octave bound;
- a cap that crosses a face edge reads both faces' blocks.
Because each pyramid level holds the range of the levels below and of the nodes, the bounds of nested caps SHALL nest (LOD-012). The plan check at production parameters is TEST-083.

Verify: A
Status: planned
Source: ADR 0088 points 3 and 8, ADR 0088/0089 accepted 2026-11-01; SPIKE-82, SPIKE-83 (the steep classes plan to level 18 with no capped tile, 2.5-4 us per lookup)

#### Scenario: Sound and nested
- **WHEN** 20,000 random caps of radius 1e-6 to 0.01 rad over a synthetic layer are bounded, rims and face edges included, each with a nested cap inside it
- **THEN** every sampled height lies in its cap's bounds, and each inner cap's bounds lie within its outer cap's (within LOD-012's slack)

## REMOVED Requirements

### Requirement: GEN-061 The fine creases follow the landform
**Reason**: ADR 0089 decision 6: the landform-keyed crease factor is removed; the amplitudes follow the class (GEN-097).
**Migration**: m9-bake PR 1, with GEN-097; the erosion field's version becomes 3.

### Requirement: GEN-062 The landform is read from a fixed lattice and bounded over a cap
**Reason**: ADR 0089 decision 1: the landform is superseded as the weight; its lattice geometry carries the class weights (GEN-093).
**Migration**: m9-bake PR 1, with GEN-093.

### Requirement: GEN-064 The hill band and the landform driver of the bake
**Reason**: ADR 0089 decisions 3, 6 and 7: the band follows the class (GEN-098) and the base elevation takes the belts and plateaus (GEN-096).
**Migration**: m9-bake PR 1, with GEN-096 and GEN-098; `GLOBAL_VERSION` becomes 6 and its pinned known answers are re-made.
