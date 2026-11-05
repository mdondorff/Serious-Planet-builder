## MODIFIED Requirements
### Requirement: GEN-013 Global-coarse field
The global-coarse stage SHALL provide one field with four channels, built per body through a generation context that records its reads (the world seed and the body's derived seed, and nothing else: not the radius, not other bodies; a missing node is an error naming it), continentalness, elevation, mean annual temperature and mean annual precipitation, as a pure function of the seed and a unit direction (integer hashing, IEEE `+ - * /` and `libm`, CON-14), under its own implementation id (`cpu-global-coarse-v1`) and version so existing goldens do not move (a change of its output bumps the version and re-records the known answers and plan snapshots that hash it, ADR 0006). Its channels SHALL be consistent: land (the stored `f32` continentalness above the sea level, which is exactly an `f32`, so readers in `f32` and `f64` agree) lies above sea level and ocean at or below it, with no step in elevation at the coast, temperature falls with latitude and, over land, with height, and precipitation follows latitude bands (a class-source world's: the orographic climate of GEN-117), the latitude being measured in COORD's axes (+z is the north pole, so the sine of the latitude is the z component of the unit direction; +x at longitude 0 and +y at longitude 90 degrees east lie on the equator). A bake SHALL evaluate it at the vertices of the six cube-face grids of a power-of-two resolution up to a stated limit, SHALL give the samples on a shared face edge equal bits, SHALL give the same result for every thread count, and SHALL refuse other resolutions naming the limit. A grid finer than the in-memory limit SHALL be baked as tile-aligned blocks (a power-of-two number of cells per block up to a stated limit, a stated finest grid and deepest level, the others refused naming the limit) handed to a consumer in a fixed order and holding at most one batch of `threads` blocks at once, with every sample equal to the level-0 lattice sample at the same position bit for bit, independent of thread count and block size. The bake SHALL be written as images (a cube net and an equirectangular map per channel) for review in `target/review/global-coarse/`, and its known answers (pointwise bit patterns and the hash of a small bake) SHALL be pinned and equal on every platform.

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

## ADDED Requirements
### Requirement: GEN-117 The orographic climate
A relief-aware climate SHALL give, at a direction, a wind vector for two seasons, the annual precipitation (the two seasons' orographic precipitation summed, read bilinearly per point), the aridity `P / PET` (`PET = 300 + 45 T`, at least 150 mm) and a seasonality value in `[0, 1]`. It SHALL be built from the base elevation alone, never from a class weight, by a fixed number of order-independent iterations (forcing in f64, f32 lattices, converging winds handled by the advection), the same bits for any thread count, never stored, and keyed by `OROGRAPHY_VERSION` and its lattice size. Precipitation SHALL be higher on a windward flank than on its lee. A class-source world's precipitation channel SHALL be this value clamped to 0 to 4,500 mm, with `GLOBAL_VERSION` bumped.

Verify: A
Status: active
Source: owner 2026-11-05, ADR 0104, SPIKE-102 F2 and F3

#### Scenario: Windward and lee
- **WHEN** the field is built on 1 and 8 threads over a synthetic ridge across the wind and over the class source, and read across a face edge
- **THEN** the bytes agree for any thread count, the windward flank is wetter than the lee by a ratio above 1.5 on the ridge, more iterations change the annual precipitation by under 2 %, an interior far from the sea is drier than a coast at the same latitude, seasonality stays in `[0, 1]`, the aridity equals `P / PET`, and the value is continuous across a face edge

### Requirement: GEN-130 The orographic climate preview
`generate --climate-preview DIR [--seed N | --world F] [--km N] [--fine-lattice N]` SHALL write, with no GPU, equirectangular maps of the annual precipitation with the annual wind alone and with two seasons, their difference, the aridity, the seasonality and the wind of both seasons with arrows, the material colour at four regions, and `orography.txt` with the land mean and quantiles of the precipitation, the land share by aridity class and by latitude band, and the windward over lee precipitation ratios across mountain belts (also on a lattice of `--fine-lattice` cells a face edge). The text and the run's line SHALL say the images show the climate, an input, not terrain; the bytes SHALL not depend on the thread count.

Verify: A
Status: active
Source: owner 2026-11-05, ADR 0104 point 5, planner inbox #75, GEN-117

#### Scenario: The preview
- **WHEN** the preview is made on 1 and 4 threads, written to a folder, and the option is combined with another generate command, a definition named twice, `--fine-lattice` alone or out of range
- **THEN** the images and the text agree for any thread count, the nine files exist, the text carries the label, the shares, the latitude bands and the ratios, and each refusal names the option
