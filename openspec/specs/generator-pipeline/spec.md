# generator-pipeline Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: GEN-001 Pure generators
A generator SHALL be a pure function of its recorded definition reads, `generator_version`, `implementation_id` and tile ID, with no global state, wall-clock time or unseeded randomness.

Verify: A
Status: active
Source: CON-06, ADR 0006

#### Scenario: Repeat
- **WHEN** the same tile is generated twice in one process and in two processes
- **THEN** the outputs are bit-identical

### Requirement: GEN-002 Thread-count independence
Generator output SHALL not depend on the number of threads, and reductions SHALL use a fixed order.

Verify: A
Status: active
Source: CON-14

#### Scenario: Thread counts
- **WHEN** a region is generated with 1, 2 and 8 threads
- **THEN** all outputs are bit-identical

### Requirement: GEN-003 Cross-OS tile hashes
A determinism manifest SHALL list output hashes for a committed set of tiles (height and mesh, every face, cube corners, levels 0 to 28) and reference-surface round trips, plus the generator, hash, mesh and key-format versions and the height and mesh keys of the same tiles for the planet and for a moon built through the generator. CI SHALL write it on Windows and Linux and fail when the two differ, naming every differing entry.

Verify: A
Status: active
Source: CON-14, ADR 0010

#### Scenario: CI comparison
- **WHEN** the determinism job diffs the Windows and Linux manifests
- **THEN** they are equal, and a difference names the kind, the tile or point and both values

#### Scenario: Different builds
- **WHEN** the manifests carry different version lines
- **THEN** the report lists the version lines first and says the manifests come from different builds

### Requirement: GEN-004 Border agreement
Neighbouring tiles SHALL agree on every shared border sample, including across cube-face edges and at the eight corners.

Verify: A
Status: active
Source: CON-08, report §16

#### Scenario: Face edge
- **WHEN** two tiles on adjacent faces are generated
- **THEN** their shared border samples are identical

### Requirement: GEN-005 Partition invariance
Generating a region as one large tile or as many small tiles SHALL give the same values at every common sample.

Verify: A
Status: active
Source: CON-08, report §16

#### Scenario: Split
- **WHEN** a tile and its four children are generated
- **THEN** samples at common positions are equal

### Requirement: GEN-006 Coarse and fine agreement
A coarse tile SHALL agree with its four children within a stated tolerance at the coarse sample positions.

Verify: A
Status: active
Source: CON-08, report §16

#### Scenario: Tolerance
- **WHEN** a parent and its children are compared
- **THEN** the maximum difference is within the tolerance and is reported when exceeded

### Requirement: GEN-007 Staged generation
Generation SHALL be staged global-coarse, regional (per tile with a halo), local (per point), and a stage SHALL read only outputs of earlier stages: a field graph SHALL be refused when a field declares a read of a field of its own stage or a later one, of an unknown field, or the same input twice, when a field is declared twice, or when a halo is not a finite fraction in [0, 1] (regional fields only; `-0.0` is `0.0`), with an error naming the fields and stages. The key of a field's output SHALL cover the stage key version, the field's stage, id, version, implementation id and halo, its scope (the whole body for the global bake, a tile for regional and local outputs, the kind part of the key, and a scope of the wrong kind for the stage a bug that panics), the body, reference surface, face mapping and resolution, the recorded read set and the keys of the outputs it reads, so that changing one field's version, implementation id or halo re-keys exactly the fields downstream of it, and a key built without one of its inputs' keys SHALL be a bug that panics rather than a stale tile. The key layout SHALL carry its own version and pinned known answers. The graph checks declared reads; a contract test that an implementation reads no more than it declares belongs to the first field that has one (ADR 0021 point 7).

Verify: A
Status: active
Source: CON-08, ADR 0006, ADR 0021 point 6, ADR 0022

#### Scenario: Illegal read
- **WHEN** a graph declares that a stage reads a later (or its own) stage's output
- **THEN** it is rejected with the field and stage names

#### Scenario: Chained keys
- **WHEN** one field's version, implementation id or halo changes
- **THEN** that field's key and the keys of every field that reads it, directly or not, change, and the keys of all other fields do not

#### Scenario: Scope
- **WHEN** a global field is keyed per tile, or a regional or local field globally
- **THEN** it panics; the tile with raw id 0 never aliases the global scope

### Requirement: GEN-008 One implementation per layer
Within one cache each field layer SHALL be produced by exactly one implementation, and neighbouring tiles SHALL share one `implementation_id`. The CPU implementation SHALL be the canonical reference, and a GPU implementation SHALL agree with it per field within `|Δ| ≤ max(rel·|v|, abs)`: heights rel 1e-6, abs 1 mm; normals 0.01°; masks abs 1e-4.

Verify: B
Status: planned
Source: CON-07, CON-14, ADR 0010

#### Scenario: Differential
- **WHEN** a GPU implementation is dispatched on test inputs
- **THEN** its read-back output matches the CPU output within the field's bound, and a failure reports the field, the tile, the sample and both values

#### Scenario: No mixed seams
- **WHEN** a cache holds tiles of one layer
- **THEN** every tile and its neighbours carry the same `implementation_id`

### Requirement: GEN-009 Field dumps
CPU code SHALL be able to write tile height maps (further field maps as fields appear) and the unfolded six-face cube net, whose cells are laid out so that shared edges are continuous, to PNG without any GPU.

Verify: A
Status: active
Source: report §16

#### Scenario: Face net
- **WHEN** the face-net dump is written
- **THEN** the PNG contains six distinct face colours and no background pixels inside the net

### Requirement: GEN-010 Mesh keys and known answers
A tile mesh SHALL carry a key covering everything its vertices depend on (tile height key, grid size, the body parameter hash with the radius, skirt fraction and a mesh version), and the mesh version SHALL be bumped whenever vertex output changes for the same inputs. Known-answer hashes of committed meshes SHALL be identical on every platform and SHALL be asserted next to the version they belong to.

Verify: A
Status: active
Source: ADR 0006, CON-14

#### Scenario: Key follows inputs
- **WHEN** the seed, tile, grid size or radius of a mesh changes
- **THEN** its key changes, and equal inputs give equal keys

#### Scenario: Known answers
- **WHEN** the committed meshes are generated
- **THEN** their content hashes equal the committed values, and the test fails if the mesh version differs from the one the values belong to

### Requirement: GEN-012 Body identity in keys
Every height key and mesh key SHALL cover the identity and parameters of the body it belongs to (ADR 0001, 0007): the body id, the id of its reference surface and, for a mesh, the body parameter hash (id, seed, radius and reference surface). Two bodies SHALL never share a key for the same tile, even with equal seeds, inputs and radii, a field built directly from a seed SHALL stand for the single planet (`BodyId(0)`), and a key composition that gains an input SHALL bump `KEY_FORMAT_VERSION` and re-record the known answers, which the determinism manifest compares between operating systems.

Verify: A
Status: active
Source: ADR 0001, ADR 0006, ADR 0007, M3 queue item 4

#### Scenario: Body alone changes the key
- **WHEN** two fields differ only in the body they describe
- **THEN** their height keys and mesh keys differ, and the planet through the default equals the field built directly from the seed

#### Scenario: Body parameters
- **WHEN** the body id, seed, radius or reference surface changes
- **THEN** the body parameter hash, and with it the mesh key, changes

### Requirement: GEN-011 Height bounds
A height field SHALL report sound bounds `(min, max)` of its height over every spherical cap (centre direction, angular radius), never excluding a height the field takes inside the cap, and the bounds of small caps SHALL follow the local relief closely enough for node volumes to hug the terrain; the default is the field's global range.

Verify: A
Status: active
Source: ADR 0016, LOD-008

#### Scenario: Soundness
- **WHEN** random points of random caps from sub-metre to several degrees are evaluated on the reference and the steep field
- **THEN** every height lies inside its cap's bounds

#### Scenario: Small caps
- **WHEN** the bounds of a cap of about 130 m are taken
- **THEN** they are tens of metres wide on the reference field, not the global range

### Requirement: GEN-013 Global-coarse field
The global-coarse stage SHALL provide one field with four channels, built per body through a generation context that records its reads (the world seed and the body's derived seed, and nothing else: not the radius, not other bodies; a missing node is an error naming it), continentalness, elevation, mean annual temperature and mean annual precipitation, as a pure function of the seed and a unit direction (integer hashing, IEEE `+ - * /` and `libm`, CON-14), under its own implementation id (`cpu-global-coarse-v1`) and version so existing goldens, plan snapshots and known answers do not move. Its channels SHALL be consistent: land (the stored `f32` continentalness above the sea level, which is exactly an `f32`, so readers in `f32` and `f64` agree) lies above sea level and ocean at or below it, with no step in elevation at the coast, temperature falls with latitude and, over land, with height, and precipitation follows latitude bands. A bake SHALL evaluate it at the vertices of the six cube-face grids of a power-of-two resolution up to a stated limit, SHALL give the samples on a shared face edge equal bits, SHALL give the same result for every thread count, and SHALL refuse other resolutions naming the limit. A grid finer than the in-memory limit SHALL be baked as tile-aligned blocks (a power-of-two number of cells per block up to a stated limit, a stated finest grid and deepest level, the others refused naming the limit) handed to a consumer in a fixed order and holding at most one batch of `threads` blocks at once, with every sample equal to the level-0 lattice sample at the same position bit for bit, independent of thread count and block size. The bake SHALL be written as images (a cube net and an equirectangular map per channel) for review in `target/review/global-coarse/`, and its known answers (pointwise bit patterns and the hash of a small bake) SHALL be pinned and equal on every platform.

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

### Requirement: GEN-014 Global bake time
A global-coarse bake at a cell size of 1 km or finer (16384 cells per face edge at Earth's radius) SHALL take no more than ten minutes on the reference machine, measured by the performance harness through the architect and never in a test (CON-20), and SHALL stay within the resident memory budget (RAM no more than 8 GB, CON-21), which the tiled bake supports by never holding more than a batch of blocks (tested at tier A under GEN-013). The harness entry is `cargo xtask perf global-bake` (ADR 0020 readiness gate, default grid 16384 cells per face edge, log `perf/logs/global-bake.json` with wall time, the most blocks alive and the bytes they take, and the store size); the architect session runs it.

Verify: C
Status: planned
Source: architecture report §14 (M3 exit criteria), CON-20, CON-21

#### Scenario: Bake timing
- **WHEN** the global bake of a 1 km grid runs on the reference machine in a perf window
- **THEN** it finishes within ten minutes and its resident memory stays within the budget

### Requirement: GEN-015 Reading the global bake
A reader of tile arrays SHALL give the global-coarse channels at a tile sample position `(k, l)` of tile `(x, y)` at level `L` with `m` cells per edge from the integer lattice index `(x m + k, y m + l)` of a face lattice of `m 2^L` cells (a halo of at most one tile edge: `k` and `l` from `-m` to `2 m`; a lattice position farther out is an error, never a clamp), never by inverting a direction (the pure-direction reader `read_direction` of GEN-016 is the second reader, ADR 0028 point 1): with the bake's own lattice of `res` cells (both powers of two) the cell and the weight are exact dyadic functions of the integer, and a position on the bake's lattice is the stored sample itself (a neighbour whose weight is zero is not read). Interpolation SHALL be bilinear in the form `a (1 - w) + b w` (along s, along t, then across) on `f64` values of the stored `f32` channels, rounded to `f32` once, so that a position on a face edge, read from either face that has it (the neighbours swap and `w` becomes `1 - w`), gives equal values (a zero compares equal to a zero of the other sign), and the same position SHALL read the same bits through any tile decomposition. A position beyond a face edge SHALL hold the edge value (ADR 0027 point 4). A bad tile shape, an out-of-range halo and a source that cannot give a vertex SHALL be errors that name what is wrong, and the reader's version (`READER_VERSION`) and known answers SHALL be pinned. Consumers SHALL decide land after interpolation from the interpolated continentalness as `f32`; the interpolated elevation is the elevation (a rule for the first regional field, tested there).

Verify: A
Status: active
Source: CON-05, CON-08, CON-14, ADR 0006, ADR 0022, ADR 0026, ADR 0027

#### Scenario: Lattice points and edges
- **WHEN** a position on the bake's lattice is read at any target resolution, or a position on a face edge between bake samples is read from each face that has it
- **THEN** the stored sample comes back (zero sign aside) and the faces agree

#### Scenario: Partition invariance
- **WHEN** one face position is read through tiles of different levels and cell counts
- **THEN** the bits are equal

#### Scenario: Halo and errors
- **WHEN** a position one sample beyond a face edge is read, or a tile shape or source is bad
- **THEN** the edge value comes back, or the error names the shape or the missing block

### Requirement: GEN-016 The terrain field
The terrain field SHALL be a pure function of the unit direction (ADR 0028): the global bake's elevation read at the direction (`read_direction`: face and face coordinates from the direction, bilinear blend of the lattice cell in floating point) plus detail noise in the direction whose amplitude is a nondecreasing function of that elevation, at most `DETAIL_MAX_M`. Every consumer SHALL call `height(dir)`, so the same world point has the same height bits from any tile, parent or child, neighbour across a face edge or cube corner, thread or consumer (COORD-007). Its `height_bounds` SHALL contain the height (also rounded to `f32`) of every direction of the cap, by the face-coordinate Lipschitz bound of ADR 0028 point 3 (faces scanned where the centre's extended coordinates are within `1 + L theta`; the global range for larger caps), and SHALL hug the terrain on small caps. The bounds SHALL add, beyond ADR 0028 point 3, `1e-6 m + 2.4e-7 |anchor|` to the slope spread (the anchor and the base at a cap point are both rounded to `f32` by the blend). The detail amplitude follows the base elevation (ocean below zero, rougher higher); it is a roughness, not a land decision, so the GEN-015 rule that consumers decide land from the interpolated continentalness is left to the first consumer that needs land (biomes, task 6), where it gets its test. A read failure of the bake SHALL give NaN from `height` and the global range from `height_bounds`, and be reported by `error()` (the first error seen; which one is kept can depend on thread timing, heights never do), never a plausible height. Its keys SHALL chain the bake: `inputs_hash` covers the bake's stage key and the persistent height key carries it as `source` (`HeightField::source_key`). The implementation id (`cpu-terrain-v1`) and `TERRAIN_VERSION` SHALL be pinned with known answers, and planning at cells 32 and viewport / tau 240 along the standard flight SHALL not be rejected for steepness (ADR 0016).

Verify: A
Status: active
Source: CON-05, CON-08, CON-14, COORD-007, ADR 0016, ADR 0022, ADR 0027, ADR 0028

#### Scenario: One point, one height
- **WHEN** the same direction is sampled by tiles of different levels, across face edges and cube corners, and from several threads
- **THEN** the height bits are equal

#### Scenario: Sound and tight bounds
- **WHEN** caps are centred at random points, near edge midpoints and at corners
- **THEN** every height in the cap lies within the bounds, and caps of a few hundred metres have bounds a few hundred metres wide

#### Scenario: Production planning
- **WHEN** the standard flight is planned at cells 32 and viewport / tau 240
- **THEN** no node is rejected for steepness
