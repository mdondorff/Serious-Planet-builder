## ADDED Requirements

### Requirement: GEN-013 Global-coarse field
The global-coarse stage SHALL provide one field with four channels, continentalness, elevation, mean annual temperature and mean annual precipitation, as a pure function of the seed and a unit direction (integer hashing, IEEE `+ - * /` and `libm`, CON-14), under its own implementation id (`cpu-global-coarse-v1`) and version so existing goldens, plan snapshots and known answers do not move. Its channels SHALL be consistent: land (the stored continentalness above the sea level) lies above sea level and ocean at or below it, with no step in elevation at the coast, temperature falls with latitude and, over land, with height, and precipitation follows latitude bands. A bake SHALL evaluate it at the vertices of the six cube-face grids of a power-of-two resolution up to a stated limit, SHALL give the samples on a shared face edge equal bits, SHALL give the same result for every thread count, and SHALL refuse other resolutions naming the limit. The bake SHALL be written as images (a cube net and an equirectangular map per channel) for review in `target/review/global-coarse/`, and its known answers (pointwise bit patterns and the hash of a small bake) SHALL be pinned and equal on every platform.

Verify: A
Status: active
Source: CON-08, CON-14, ADR 0006, ADR 0022, architecture report §14 (M3 exit criteria)

#### Scenario: Thread independence
- **WHEN** the same seed is baked with 1, 2, 3, 7 and 64 threads
- **THEN** every sample has the same bits and equals the pointwise field at the same lattice direction

#### Scenario: Face edges
- **WHEN** vertices of different faces are the same direction
- **THEN** their samples are equal

#### Scenario: A world
- **WHEN** a 64-cell bake (and a 128-cell one for the lapse rate) is inspected
- **THEN** it has continents and oceans, deep ocean and high mountains, a warm equator and cold poles, colder high ground and a wetter equatorial belt than subtropics

#### Scenario: Products
- **WHEN** the bake is rendered per channel
- **THEN** the cube net has the six faces on the known cross layout and is not flat, and the images are written for review

### Requirement: GEN-014 Global bake time
A global-coarse bake at a cell size of 1 km or finer (16384 cells per face edge at Earth's radius) SHALL take no more than ten minutes on the reference machine, measured by the performance harness through the architect and never in a test (CON-20), and SHALL not hold the whole grid in memory.

Verify: C
Status: planned
Source: architecture report §14 (M3 exit criteria), CON-20, CON-21

#### Scenario: Bake timing
- **WHEN** the global bake of a 1 km grid runs on the reference machine in a perf window
- **THEN** it finishes within ten minutes and its resident memory stays within the budget
