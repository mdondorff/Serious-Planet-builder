## ADDED Requirements

### Requirement: GEN-104 The continent stage
The global stage's continentalness SHALL be the continent stage of `planet-generators`, a pure function of the seed and the unit direction, `GLOBAL_VERSION` 6. Its raw value is the sum of:
- the shape: an eight-octave fBm of base frequency 1.6 and gain 0.72 per octave (each octave its own layer), read at the direction plus 0.6 times a three-octave warp of frequency 1.3, times 1.7, times `0.65 + 0.35 * 4 w (1 - w)`;
- the craton bias `0.8 * sum` of the lobe weights `smoothstep(0, 0.25, z) exp(-(u^2 + v^2))`, `z` the cosine of the angle to the lobe's centre (the weight is 0 behind it, and fades over `z < 0.25` so that the bias is continuous across the great circle 90 degrees from the centre), `u` and `v` the tangent angles `atan2` of the direction's components along the lobe's major and minor axes and `z`, over its radii, read at the direction plus 0.3 times the warp, with `w = min(1, sum)`;
- the islands (GEN-106).
A craton is a chain of lobes; the cratons hold the land shares 0.50, 0.26, 0.10, 0.06, 0.03, 0.02, 0.01, 0.01 of 29 % of the sphere (a cap of half-angle `r` holds `2 pi (1 - cos r)`), the two largest (0.50 and 0.26, at least 20 % of the land) as 3 lobes, a craton of 4 to 20 % as 1 to 3 (drawn), the others one; a lobe has aspect `a` uniform in 1.5 to 2.5 (radii `r sqrt(a)` and `r / sqrt(a)`), lobe centres 0.6 times the sum of the two major radii apart on a spine that turns by up to 1.2 rad per lobe, and craton centres at least the two extents plus 0.1 rad apart (up to 200 draws). The per-seed offset is the `1 - 0.29` quantile of the raw value over 20,000 directions of a Fibonacci sphere; the continentalness is the raw value minus the offset plus the sea level. The octave series is normalised by its full weight sum, so that a value with fewer octaves is the full value without its detail terms. Two such values SHALL be exposed: the smooth value (the first 4 octaves, no islands) for the distances of the class source (GEN-094), and the coarse value (the first 3 octaves, no islands) for the landform driver. The landform driver and the hill band's strength read the coarse value (GEN-064, whose text this change edits in `openspec/specs`, synced here because `m9-bake` removes GEN-064 and its delta cannot also modify it). `GLOBAL_VERSION` is 6 (`m9-bake`'s GEN-096 makes it 7). Every term SHALL read the 3D direction (no latitude or longitude), use `libm` and integer hashing only, and give the same bits for any thread count (CON-14). The constants are start values (SPIKE-92); R7 tunes them.

Verify: A
Status: active
Source: SPIKE-92 round 3 (H2r3), planner (2026-11-02, inbox #41), ADR 0089 decision 10

#### Scenario: Pure and thread-independent
- **WHEN** the continentalness of seed 14 is evaluated on 1 and on 8 threads at 2000 directions, including the poles and cube edges
- **THEN** the bits are equal

### Requirement: GEN-105 The craton layout and plate draws are retried deterministically
The continent stage SHALL draw its craton layout from the seed, then from derived seeds (draw `t` = 1 to 11 uses `seed xor (0x53927e7e * (t + 1))`, 12 draws in all), until a coarse land map (2 degree cells, 29 % of the area by the raw value's quantile, 8-connected, longitude wrapping) has 4 to 7 masses of at least 1 Mkm2 and the largest holds 47 to 60 % of the land; else it SHALL keep the draw with the lowest cost `max(0, 4 - n) + max(0, n - 7) + 10 (max(0, 0.47 - s) + max(0, s - 0.60))`, the first on ties. The class source SHALL draw its plates by the same kind of retry (GEN-094): up to 64 draws, draw 0 from the seed and draw `t` = 1 to 63 from `splitmix(seed xor (t << 40) xor 0x71a7e500)`, the first draw with the highest score kept. The island term (GEN-106) is part of the layout score, so changing it, or any constant of the shape, is a layout change: the kept draw of seeds 1 to 5 is pinned (draws 2, 1, 1, 2, 3 for seeds 1 to 5 and 1 for seed 14, the reference world) and a swap needs a declared re-pin. Sorts SHALL use a total order with an index tie-break; the grids and the draw order are fixed (CON-14).

Verify: A
Status: active
Source: SPIKE-92 (the layout retry), planner (2026-11-02, inbox #41); belts on the continents (round 2)

#### Scenario: A retry is a pure function of the seed
- **WHEN** the continentalness and the class weights of seed 2 are built twice, on different thread counts
- **THEN** the same draws are kept and the bits are identical

### Requirement: GEN-106 Islands lie off the craton margins on a smaller warp
The islands term SHALL be `1.3 * near * f`, with `near = exp(-((w - 0.2) / 0.15)^2)` (`w` the clamped lobe sum of GEN-104: islands lie just offshore of the craton margins) and `f` a four-octave fBm of base frequency 3.0 and gain 0.6, read at the direction plus 0.3 times the warp (half the shape's), so that the high-frequency noise is not stretched into parallel striations.

Verify: B
Status: active
Source: SPIKE-92 round 3 (island fields' striations), planner (2026-11-02, inbox #41)

#### Scenario: Islands
- **WHEN** the land mask of seeds 1 to 5 is scored on a 1440 x 720 grid
- **THEN** TEST-088's island band holds

## MODIFIED Requirements

### Requirement: GEN-021 Climate field of a tile
A tile SHALL carry its climate (ADR 0039 point 2): the temperature in degrees C and the precipitation in mm per year at 9 x 9 points, texel `(a, b)` at tile sample `(a, b)` of a tile grid of 8 cells, two f16 values per texel, 324 bytes per tile, (the compact tile's climate block in its slot, GEN-025). It is a **field** (ADR 0035 point 1): it is produced by a generator, stored and keyed with the tile and read by the renderer, so it has its own implementation id (`cpu-climate-v1`) and version, and one implementation per cache with the CPU canonical (CON-07). Its values come from the global bake read with the integer reader of ADR 0027 (`read_tile_sample`, by every field over a bake: the terrain field and the eroded field; a wrapper of a field, such as the streamer's cancellable one, delegates `climate`, `climate_version_word` and `heights_and_climate` so a tile built through it has the key and the climate of the inner field; lattice position from integers, face-edge-symmetric interpolation, `READER_VERSION`); a field without a bake (the reference generator, synthetic fields) takes the global-coarse climate formula (`global::climate_from_elevation`: temperature from latitude and altitude, rain from the latitude bands, no inland measure) over its OWN seed and OWN heights at the same directions, a pure function of `(seed, mapping, tile, heights)` and not of the generator's world (so a change of the generator, `GLOBAL_VERSION` included, moves no such field's climate, its keys or the renderer goldens drawn from it). The generator's own climate over no bake (`analytic_climate`, the continent stage's elevation) is the climate of the generator's fields, which read it through the bake. Both producers measure latitude in COORD's axes: +z is the north pole, so at +z the climate is that of latitude 90, at -z that of latitude -90 and at +x and +y that of the equator (the analytic function once took the latitude from the y component, which put its poles at +-y; fixed with `GLOBAL_VERSION` 3, whose bump re-keys every tile through the version word). The stored value SHALL be the f16 rounding (round to nearest, ties to even) of the reader's f32 value, computed with integer arithmetic and `libm` only, so the CPU result is the definition of the field, the implementation id `cpu-climate-v1` is part of the version word, the GPU decode (`unpack2x16float`) is exact and CON-14's CPU/GPU bound for this field is zero; against the bake the rounding costs at most 2^-11 of the value (declared quantisation: half a step, at most 0.008 degrees below 16 C and 0.016 below 32 C, 2 mm of precipitation below 2,048 mm and 4 mm below 4,096 mm; the field's range is 0 to 4,500 mm), a deviation from ADR 0039's "abs 0.01 degrees and 0.1 mm" that f16 cannot meet for large values, recorded here. The version word that enters every mesh key and persistent mesh key (`PARAM_CLIMATE_VERSION`) of a field over a bake SHALL cover the climate version, `READER_VERSION` and `GLOBAL_VERSION`; that of a field of its own heights covers the climate version and the version of the height-climate function (`HEIGHT_CLIMATE_VERSION` 1) and not `GLOBAL_VERSION`. Texels SHALL be bit-identical across runs, thread counts and operating systems, with known answers, neighbouring tiles SHALL share their border texels bit for bit (also across face edges), and an unreadable block SHALL be an error naming it (recorded like a height's, never a default).

Verify: A, C
Status: active
Source: ADR 0039 point 2, ADR 0027, ADR 0035, ADR 0033, CON-07, CON-14, CON-05, GEN-025, ADR 0062

#### Scenario: f16 rounding
- **WHEN** every finite f16 is decoded and re-encoded, and values at ties, at the largest finite value, at the subnormal range and beyond it are encoded
- **THEN** every round trip is exact, ties go to even, overflow gives infinity and the error of a normal value is at most 2^-11 relative

#### Scenario: Values from the bake
- **WHEN** the climate of tiles at several levels and faces is built over a bake and compared with `read_tile_sample` at the 81 positions
- **THEN** each texel is the f16 rounding of the reader's temperature and precipitation

#### Scenario: Shared borders
- **WHEN** neighbouring tiles of one face and tiles of different faces that share a border are built
- **THEN** texels at bit-equal directions are bit-equal

#### Scenario: Determinism
- **WHEN** the climate of 24 tiles is built on 1, 5 and 24 threads
- **THEN** the content hashes are equal to each other and to the committed known answers, and the version word equals its committed value

#### Scenario: Storage
- **WHEN** a compact tile of a field is built
- **THEN** it carries the climate of the field at the tile's 81 points, its climate block is 324 bytes in its slot, its texels are 8,712 bytes and hold no climate, and a field with another climate version word has another mesh key

#### Scenario: Fields over a bake
- **WHEN** the terrain field and the eroded field are built over a bake, and a tile is built through the streamer's cancellable wrapper of each
- **THEN** their climate is the reader's (not the analytic function's), their version word is the bake's, and the wrapped tile equals the unwrapped one including its key and climate

#### Scenario: Interpolation
- **WHEN** the texture is sampled at texel positions and between them
- **THEN** a texel position gives the texel, a midpoint the mean, and a vertex at a texel position takes the texel's value

#### Scenario: Latitude axis
- **WHEN** the climate texture is built (analytically and over a bake) for the tile at the +z pole, the tile at the -z pole and a tile on the equator at +x
- **THEN** the temperature texels at both poles are at least 30 degrees C colder than the equator's, and the version word names the current `GLOBAL_VERSION`

#### Scenario: A field of its own heights
- **WHEN** the climate of the reference field is built for a tile, and for a field of a constant height at 0 m and at 2,000 m
- **THEN** the reference field's texels are the climate formula over its own heights and differ from the generator's global-coarse climate somewhere, the temperature of the constant fields differs by 13 degrees (6.5 per km, up to the f16 rounding) and their rain is equal
