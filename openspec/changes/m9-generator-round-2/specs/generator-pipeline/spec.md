## ADDED Requirements

### Requirement: GEN-104 The continent stage
The global stage's continentalness SHALL be the continent stage of `planet-generators`, a pure function of the seed and the unit direction, `GLOBAL_VERSION` 6. Its raw value is the sum of:
- the shape: an eight-octave fBm of base frequency 1.6 and gain 0.72 per octave (each octave its own layer), read at the direction plus 0.6 times a three-octave warp of frequency 1.3, times 1.7, times `0.65 + 0.35 * 4 w (1 - w)`;
- the craton bias `0.8 * sum` of the lobe weights `smoothstep(0, 0.25, z) exp(-(u^2 + v^2))`, `z` the cosine of the angle to the lobe's centre (the weight is 0 behind it, and fades over `z < 0.25` so that the bias is continuous across the great circle 90 degrees from the centre), `u` and `v` the tangent angles `atan2` of the direction's components along the lobe's major and minor axes and `z`, over its radii, read at the direction plus 0.3 times the warp, with `w = min(1, sum)`;
- the islands (GEN-106).
A craton is a chain of lobes; the cratons hold the land shares 0.50, 0.26, 0.10, 0.06, 0.03, 0.02, 0.01, 0.01 of 29 % of the sphere (a cap of half-angle `r` holds `2 pi (1 - cos r)`), the two largest (0.50 and 0.26, at least 20 % of the land) as 3 lobes, a craton of 4 to 20 % as 1 to 3 (drawn), the others one; a lobe has aspect `a` uniform in 1.5 to 2.5 (radii `r sqrt(a)` and `r / sqrt(a)`), lobe centres 0.6 times the sum of the two major radii apart on a spine that turns by up to 1.2 rad per lobe, and craton centres at least the two extents plus 0.1 rad apart (up to 200 draws). The per-seed offset is the `1 - 0.29` quantile of the raw value over 20,000 directions of a Fibonacci sphere; the continentalness is the raw value minus the offset plus the sea level. The octave series is normalised by its full weight sum, so that a value with fewer octaves is the full value without its detail terms. Two such values SHALL be exposed: the smooth value (the first 4 octaves, no islands) for the distances of the class source (GEN-094), and the coarse value (the first 3 octaves, no islands) for the landform driver. The landform driver and the hill band's strength read the coarse value (GEN-064, whose text this change edits in `openspec/specs`, synced here because `m9-bake` removes GEN-064 and its delta cannot also modify it). `GLOBAL_VERSION` is 6 (`m9-bake`'s GEN-096 makes it 7). Every term SHALL read the 3D direction (no latitude or longitude), use `libm` and integer hashing only, and give the same bits for any thread count (CON-14). The constants are start values (SPIKE-92); R7 tunes them.

Verify: A
Status: planned
Source: SPIKE-92 round 3 (H2r3), planner (2026-11-02, inbox #41), ADR 0089 decision 10

#### Scenario: Pure and thread-independent
- **WHEN** the continentalness of seed 3 is evaluated on 1 and on 8 threads at 2000 directions, including the poles and cube edges
- **THEN** the bits are equal

### Requirement: GEN-105 The craton layout and plate draws are retried deterministically
The continent stage SHALL draw its craton layout from the seed, then from derived seeds (draw `t` = 1 to 11 uses `seed xor (0x53927e7e * (t + 1))`, 12 draws in all), until a coarse land map (2 degree cells, 29 % of the area by the raw value's quantile, 8-connected, longitude wrapping) has 4 to 7 masses of at least 1 Mkm2 and the largest holds 47 to 60 % of the land; else it SHALL keep the draw with the lowest cost `max(0, 4 - n) + max(0, n - 7) + 10 (max(0, 0.47 - s) + max(0, s - 0.60))`, the first on ties. The class source SHALL draw its plates by the same kind of retry (GEN-094): up to 16 draws, draw 0 from the seed and draw `t` = 1 to 15 from `splitmix(seed xor (t << 40) xor 0x71a7e500)`, the first draw with the highest score kept. The island term (GEN-106) is part of the layout score, so changing it, or any constant of the shape, is a layout change: the kept draw of seeds 1 to 5 is pinned (draws 2, 1, 1, 2, 3; seed 3 is the reference world) and a swap needs a declared re-pin. Sorts SHALL use a total order with an index tie-break; the grids and the draw order are fixed (CON-14).

Verify: A
Status: planned
Source: SPIKE-92 (the layout retry), planner (2026-11-02, inbox #41); belts on the continents (round 2)

#### Scenario: A retry is a pure function of the seed
- **WHEN** the continentalness and the class weights of seed 2 are built twice, on different thread counts
- **THEN** the same draws are kept and the bits are identical

### Requirement: GEN-106 Islands lie off the craton margins on a smaller warp
The islands term SHALL be `1.3 * near * f`, with `near = exp(-((w - 0.2) / 0.15)^2)` (`w` the clamped lobe sum of GEN-104: islands lie just offshore of the craton margins) and `f` a four-octave fBm of base frequency 3.0 and gain 0.6, read at the direction plus 0.3 times the warp (half the shape's), so that the high-frequency noise is not stretched into parallel striations.

Verify: B
Status: planned
Source: SPIKE-92 round 3 (island fields' striations), planner (2026-11-02, inbox #41)

#### Scenario: Islands
- **WHEN** the land mask of seeds 1 to 5 is scored on a 1440 x 720 grid
- **THEN** TEST-088's island band holds
