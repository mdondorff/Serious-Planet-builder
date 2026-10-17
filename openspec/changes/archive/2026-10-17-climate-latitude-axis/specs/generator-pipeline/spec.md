## MODIFIED Requirements

### Requirement: GEN-013 Global-coarse field
The global-coarse stage SHALL provide one field with four channels, built per body through a generation context that records its reads (the world seed and the body's derived seed, and nothing else: not the radius, not other bodies; a missing node is an error naming it), continentalness, elevation, mean annual temperature and mean annual precipitation, as a pure function of the seed and a unit direction (integer hashing, IEEE `+ - * /` and `libm`, CON-14), under its own implementation id (`cpu-global-coarse-v1`) and version so existing goldens do not move (a change of its output bumps the version and re-records the known answers and plan snapshots that hash it, ADR 0006). Its channels SHALL be consistent: land (the stored `f32` continentalness above the sea level, which is exactly an `f32`, so readers in `f32` and `f64` agree) lies above sea level and ocean at or below it, with no step in elevation at the coast, temperature falls with latitude and, over land, with height, and precipitation follows latitude bands, the latitude being measured in COORD's axes (+z is the north pole, so the sine of the latitude is the z component of the unit direction; +x at longitude 0 and +y at longitude 90 degrees east lie on the equator; bake version 3, the first to do so: version 2 took it from the y component). A bake SHALL evaluate it at the vertices of the six cube-face grids of a power-of-two resolution up to a stated limit, SHALL give the samples on a shared face edge equal bits, SHALL give the same result for every thread count, and SHALL refuse other resolutions naming the limit. A grid finer than the in-memory limit SHALL be baked as tile-aligned blocks (a power-of-two number of cells per block up to a stated limit, a stated finest grid and deepest level, the others refused naming the limit) handed to a consumer in a fixed order and holding at most one batch of `threads` blocks at once, with every sample equal to the level-0 lattice sample at the same position bit for bit, independent of thread count and block size. The bake SHALL be written as images (a cube net and an equirectangular map per channel) for review in `target/review/global-coarse/`, and its known answers (pointwise bit patterns and the hash of a small bake) SHALL be pinned and equal on every platform.

Verify: A
Status: active
Source: CON-06, CON-08, CON-14, ADR 0006, ADR 0022, architecture report §14 (M3 exit criteria)

#### Scenario: Thread independence
- **WHEN** the same seed is baked with 1, 2, 3, 7 and 64 threads
- **THEN** every sample has the same bits and equals the pointwise field at the same lattice direction

#### Scenario: Face edges
- **WHEN** vertices of different faces are the same direction
- **THEN** their samples are equal

#### Scenario: A world
- **WHEN** a 64-cell bake (and a 128-cell one for the lapse rate) is inspected
- **THEN** it has continents and oceans, deep ocean and high mountains, a warm equator and cold poles, colder high ground and a wetter equatorial belt than subtropics

#### Scenario: Blocks
- **WHEN** a grid is baked as blocks with 1 or 5 threads, or the lattice of a 256-cell face is inspected
- **THEN** the blocks arrive in the same order with the same bits, at most `threads` of them are alive at once, a block equals its window of the level-0 lattice, and the cube's edge directions are exactly 12 (res - 1) + 8 distinct ones

#### Scenario: Sea level
- **WHEN** a continentalness of exactly `0.1f32` is stored
- **THEN** it is sea for an `f32` reader and for the bake alike, and the next `f32` up is land

#### Scenario: Products
- **WHEN** the bake is rendered per channel
- **THEN** the cube net has the six faces on the known cross layout and is not flat, and the images are written for review

#### Scenario: Latitude axis
- **WHEN** the global-coarse field is sampled at +z, -z, +x and +y, and over a bake and an equirectangular map
- **THEN** the sine of the latitude is 1, -1, 0 and 0, the poles at +-z are the coldest places of every seed, the equator is warm, and the top and bottom rows of the equirectangular map are the poles

### Requirement: GEN-021 Climate field of a tile
A tile SHALL carry its climate (ADR 0039 point 2): the temperature in degrees C and the precipitation in mm per year at 9 x 9 points, texel `(a, b)` at tile sample `(a, b)` of a tile grid of 8 cells, two f16 values per texel, 324 bytes per tile, in both tile formats (the compact tile's spare channel, GEN-019; the vertex tile's own 324-byte block, STRM-021). It is a **field** (ADR 0035 point 1): it is produced by a generator, stored and keyed with the tile and read by the renderer, so it has its own implementation id (`cpu-climate-v1`) and version, and one implementation per cache with the CPU canonical (CON-07). Its values come from the global bake read with the integer reader of ADR 0027 (`read_tile_sample`, by every field over a bake: the terrain field and the eroded field; a wrapper of a field, such as the streamer's cancellable one, delegates `climate`, `climate_version_word` and `heights_and_climate` so a tile built through it has the key and the climate of the inner field; lattice position from integers, face-edge-symmetric interpolation, `READER_VERSION`); a field without a bake (the reference generator, synthetic fields) takes the analytic global-coarse function of its seed at the same directions, a pure function of `(seed, mapping, tile)`. Both producers measure latitude in COORD's axes: +z is the north pole, so at +z the climate is that of latitude 90, at -z that of latitude -90 and at +x and +y that of the equator (the analytic function once took the latitude from the y component, which put its poles at +-y; fixed with `GLOBAL_VERSION` 3, whose bump re-keys every tile through the version word). The stored value SHALL be the f16 rounding (round to nearest, ties to even) of the reader's f32 value, computed with integer arithmetic and `libm` only, so the CPU result is the definition of the field, the implementation id `cpu-climate-v1` is part of the version word, the GPU decode (`unpack2x16float`) is exact and CON-14's CPU/GPU bound for this field is zero; against the bake the rounding costs at most 2^-11 of the value (declared quantisation: half a step, at most 0.008 degrees below 16 C and 0.016 below 32 C, 2 mm of precipitation below 2,048 mm and 4 mm below 4,096 mm; the field's range is 0 to 4,500 mm), a deviation from ADR 0039's "abs 0.01 degrees and 0.1 mm" that f16 cannot meet for large values, recorded here. The version word that enters every mesh key and persistent mesh key (`PARAM_CLIMATE_VERSION`) SHALL cover the climate version, `READER_VERSION` and `GLOBAL_VERSION`. Texels SHALL be bit-identical across runs, thread counts and operating systems, with known answers, neighbouring tiles SHALL share their border texels bit for bit (also across face edges), and an unreadable block SHALL be an error naming it (recorded like a height's, never a default).

Verify: A, B
Status: active
Source: ADR 0039 point 2, ADR 0027, ADR 0035, ADR 0033, CON-07, CON-14, CON-05

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
- **WHEN** a compact tile and a vertex tile of one field and tile are built
- **THEN** both carry the same climate, the compact tile's texels are still 8,712 bytes, and a field with another climate version word has another mesh key

#### Scenario: Fields over a bake
- **WHEN** the terrain field and the eroded field are built over a bake, and a tile is built through the streamer's cancellable wrapper of each
- **THEN** their climate is the reader's (not the analytic function's), their version word is the bake's, and the wrapped tile equals the unwrapped one including its key and climate, in both formats

#### Scenario: Interpolation
- **WHEN** the texture is sampled at texel positions and between them
- **THEN** a texel position gives the texel, a midpoint the mean, and a vertex at a texel position takes the texel's value

#### Scenario: Latitude axis
- **WHEN** the climate texture is built (analytically and over a bake) for the tile at the +z pole, the tile at the -z pole and a tile on the equator at +x
- **THEN** the temperature texels at both poles are at least 30 degrees C colder than the equator's, and the version word names `GLOBAL_VERSION` 3
