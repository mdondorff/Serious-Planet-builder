# generator-pipeline Specification

## Purpose
The generators that turn a WorldDefinition into fields: pure, thread-count-independent and bit-identical across operating systems (CON-06, CON-14, ADRs 0006, 0010, 0023), staged global-coarse, regional and local with border, partition and coarse/fine agreement (CON-08, ADRs 0021, 0022), one implementation per layer (CON-07), and the global bake and the terrain and eroded height fields built on it (ADRs 0026-0028, 0030).

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
A determinism manifest SHALL list output hashes for a committed set of tiles (height and mesh, every face, cube corners, levels 0 to 28) and reference-surface round trips, plus the generator, hash, mesh and key-format versions and the height and mesh keys of the same tiles for the planet and for a moon built through the generator. It SHALL also list the global bake at 64 cells per face edge (the whole-grid hash and the hash of every block of 16 cells at level 2), the raw `f64` height of the terrain field (`cpu-terrain-v1`) and of the eroded field (`cpu-erosion-v1`) at twelve face-coordinate points on every face (the four cube corners, the four edge midpoints, the centre and three further points on and near edges and corners), the component table and the channel of the ocean connectivity of that bake (`oceantable`, `oceangrid`, GEN-022), the versions and implementation ids of the bake and both fields, and the persistent known answers of ADR 0018 (keys, node, body and query hashes, the bake's stage and block keys, an ocean block's key and the node set id; written by `app`, ADR 0023 point 9). A one-ulp change in the value behind any of the new entries SHALL change that entry. CI SHALL write it on Windows and Linux and fail when the two differ, naming every differing entry.

Verify: A
Status: active
Source: CON-14, ADR 0010, ADR 0023

#### Scenario: CI comparison
- **WHEN** the determinism job diffs the Windows and Linux manifests
- **THEN** they are equal, and a difference names the kind, the tile or point and both values

#### Scenario: Mutation of a new entry
- **WHEN** the value behind any bake, block, terrain or eroded entry is changed by one ulp
- **THEN** exactly that line of the manifest changes

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
A tile mesh SHALL carry a key covering everything its vertices depend on (tile height key, grid size, the body parameter hash with the radius, skirt fraction and a mesh version), and the mesh version SHALL be bumped whenever the CPU mesh output changes for the same inputs, the compact construction (GEN-019) changes or the key composition changes; version 5 introduced the compact tile, which carries the mesh key of its tile; version 6 added the finest-level class (LOD-010: a tile at or deeper than the view's finest level had its key rehashed with the class, because the format of a finest tile was decided from its relief), which `remove-vertex-render-path` removes without a version bump (ADR 0062: no decision depends on the relief any more, so a tile's key is its base key at every level; the class key only named in-memory tiles at the cap, and the persistent pack never stores a tile at or above the cap, STRM-021, so no stored record can answer to a key whose meaning changed); version 7 adds the climate field (GEN-021): a tile carries 9 x 9 f16 texels of temperature and precipitation, and the key names the climate version word (the climate, reader and bake versions) so a change of any of them re-keys every tile, while the vertex output and the compact geometry are unchanged (the committed mesh hashes, which do not cover the climate, stay as they were under version 6); version 8 changes the compact tile to the slot of GEN-025 (wide texel, climate block and record inside the slot): every key changes (the version is a key input), the content of the CPU meshes and the committed mesh hashes stay as they were. The plan's keys and the compact tiles of one view SHALL agree: a tile has one key whatever the finest level of the view (the same tile under another cap is the same tile with the same key). Persisted height tiles (STRM-009) are not meshes and are unchanged by it; persistent mesh keys (kind mesh) change with the version, and repro bundles of an older mesh version are refused with the versions named (TEST-008). Known-answer hashes of committed meshes SHALL be identical on every platform and SHALL be asserted next to the version they belong to.

Verify: A
Status: active
Source: ADR 0006, CON-14, LOD-010, GEN-025, ADR 0062

#### Scenario: Key follows inputs
- **WHEN** the seed, tile, grid size or radius of a mesh changes
- **THEN** its key changes, and equal inputs give equal keys

#### Scenario: Finest-level class
- **WHEN** the plan of a view is built with the cap 22 and with the cap 23
- **THEN** every draw tile has its base key under both caps (the class key is gone, ADR 0062), and the level-22 tile has the same key in both plans

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

### Requirement: GEN-014 Global bake time
A global-coarse bake at a cell size of 1 km or finer (16384 cells per face edge at Earth's radius) SHALL take no more than ten minutes on the reference machine, measured by the performance harness through the architect and never in a test (CON-20), and SHALL stay within the resident memory budget (RAM no more than 8 GB, CON-21), which the tiled bake supports by never holding more than a batch of blocks (tested at tier A under GEN-013). The `global-bake` perf scene (`perf_global_bake`) cites this requirement for spec-lint (TEST-001). The harness entry is `cargo xtask perf global-bake` (ADR 0020 readiness gate, default grid 16384 cells per face edge, log `perf/logs/global-bake.json` with wall time, the most blocks alive and the bytes they take, and the store size); the architect session runs it.

Verify: C
Status: active
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

### Requirement: GEN-017 The eroded terrain field
The eroded terrain field SHALL be a pure function of the unit direction (ADR 0030 point 1, SPIKE-04 variant B), under its own implementation id (`cpu-erosion-v1`) and `EROSION_VERSION`, built from the same bake and keyed like the terrain field (GEN-016: the bake's stage key in `inputs_hash` and as `source`), plus the body's radius, which is read through the context (the slope is rise over run). Its height SHALL be the bake's elevation `E(dir)` plus `a(E) c(slope) S(dir)`: `a` the crease amplitude (80 m plus up to 900 m with the elevation), `c` a slope factor from 0.15 on flat ground to 1 at a slope of 0.2, `S` in [-0.75, 0.25] the weighted mean over 12 octaves of `0.25 - (1 - |n|)^3` with `n` the octave's lattice noise. The slope SHALL be the magnitude of a gradient of `E` measured over three bake cells either side of the direction as the sum, over the three coordinate axes, of the central difference along the tangent part `t` of the axis times `t` (independent of a tangent basis, so no seam where a basis would switch). `height_bounds` SHALL contain the height (also rounded to `f32`) of every direction of the cap by interval arithmetic: the bake's range from the lattice cells the cap can read (GEN-016), the slope interval from the gradient at the centre and a bound on its variation over the cap (`2.5 L K r (1 / delta + 2)` plus the `f32` rounding of the bake over `delta`: `L` the bake's Lipschitz constant over the cap widened by the stencil, `K` the Lipschitz constant of the stencil map, `r` the cap radius, `delta` the stencil half-width, 2.5 a bound on the sum of the tangent parts' lengths, and the `2` the movement of the tangent part with the point; capped by the steepest slope the bake allows), the amplitude interval from the elevation and slope-factor intervals (both nondecreasing), the crease interval from the per-octave noise bounds, and their product; a cap too wide for the terrain field's scan takes the global range, and one whose widened slope scan fails takes the full range of the slope factor. A read failure of the bake SHALL give NaN from `height` and the global range from `height_bounds` and be reported by `error()`. Planning at cells 32 and viewport / tau 240 along the standard flight SHALL not be rejected for steepness with these bounds, on the test bake and (at M3 acceptance) on a production-resolution bake. The production-resolution run is a recorded acceptance item, not a test: `generate --plan-check DIR --global-res N` plans the flight over a stored bake with the terrain and the eroded field, and its output is kept in `docs/milestones/M3-plan-check-16384.txt`.

Verify: A
Status: active
Source: CON-05, CON-14, COORD-007, ADR 0016, ADR 0022, ADR 0028, ADR 0030

#### Scenario: Sound bounds across a steep ramp
- **WHEN** caps are centred at random points, near edge midpoints and corners, and on a lattice whose slope factor changes within the cap
- **THEN** every height of the cap lies within the bounds

#### Scenario: Tight bounds on small caps
- **WHEN** caps of 640 m are bounded on the test bake
- **THEN** the median bounds are a few tens of metres wide

#### Scenario: Steeper ground is cut deeper
- **WHEN** the same creases are applied on a steep ramp and on flat ground
- **THEN** the mean incision on the ramp is more than three times that on flat ground

#### Scenario: Production planning
- **WHEN** the standard flight is planned with the eroded field at cells 32 and viewport / tau 240
- **THEN** no node is rejected for steepness

### Requirement: GEN-018 The world names its height field
A world definition SHALL name its height field by implementation id (`world.height_field`). Resolution SHALL give the eroded field (`cpu-erosion-v1`, GEN-017) when the definition names none, the named field when the id is known (`cpu-erosion-v1`, `cpu-terrain-v1`, `cpu-ref-height-v2`), and otherwise an error that names the unknown id and lists every known id. The field built for a resolved id SHALL report that id as its `implementation_id`. Every path of `test-render --scene terrain` (a single view, `--flight`, `--stream`, `--bundle`, `--perf` and `--perf-smoke`) SHALL build its field from the world's resolution (`--world`, an unknown id being refused before any GPU work) over an in-memory bake of `--global-res` cells per face edge (default 256, at most 1024); `--repro` SHALL rebuild the field from the world the bundle records (TEST-008) and refuse `--world`. A view, stream, flight or bundle that names no world file is the seed-only world and SHALL name `cpu-ref-height-v2` explicitly, so no existing golden, plan snapshot or repro bundle changes; the perf scene that names no world file SHALL run the default world of `--seed` (the eroded field), because it measures the world a user gets, and SHALL report which field and bake it measured. A streamed mesh SHALL equal the synchronously generated mesh of the same field. The interactive editor has no window yet.

Verify: A
Status: active
Source: owner's decision of 2026-10-13 (M4 item 0) and of 2026-10-15 (M4 item 7d), ADR 0028, ADR 0030, CON-06, CON-07

#### Scenario: Default
- **WHEN** a definition names no height field
- **THEN** it resolves to `cpu-erosion-v1`

#### Scenario: Named
- **WHEN** a definition names `cpu-terrain-v1`
- **THEN** it resolves to `cpu-terrain-v1`, and the built field reports that id

#### Scenario: Unknown id
- **WHEN** a definition names an id no build knows
- **THEN** resolution fails with an error that contains the id and the list of known ids

#### Scenario: Every terrain path takes the world
- **WHEN** `--stream`, `--bundle`, `--perf` or `--perf-smoke` is given with `--world`, and when the perf scene is given none
- **THEN** the options are accepted, the field is the world's (the perf scene without a world: the eroded field over the `--global-res` bake), and a view with `--global-res` and no world is refused

#### Scenario: Streamed equals synchronous
- **WHEN** a terrain view with a world is rendered with and without `--stream`
- **THEN** the two frames are identical

### Requirement: GEN-019 Compact tile
The tile builder SHALL produce, for levels 3 and finer, a compact tile (ADR 0033 point 4): one 8-byte texel per grid vertex (`(cells + 1)^2` texels: a 32-bit height step count between the tile's own minimum and maximum, then in the second word two signed 15-bit octahedral normal components in bits 0-14 and 15-29, bits 30-31 zero), the tile's climate block (GEN-021: 81 words, 324 bytes) and a 128-byte header (nine tile-local control points of a biquadratic base surface on the shell through the tile origin, the height range `hr_min` and `hr_step`, the origin height, the skirt depth and the flags: bit 0 the orientation flip, every other bit zero), laid out as one slot of 9,168 bytes at cells 32 (GEN-025; `MESH_VERSION` 8). A compact tile keeps the minimum of 16 cells per tile edge (a build with fewer cells is refused naming the minimum; the former reason, 162 climate values in 162 texels, is gone, the minimum stays so that the cells 16 configurations and their pool arithmetic are unchanged). A tile's key SHALL be the mesh key of its tile (GEN-010). The stored normal SHALL equal the canonical CPU normal within 0.01 degrees (CON-14); the height SHALL be reconstructed as `hr_min + f32(q) * hr_step` with the step the relief over 16,777,215 (2^24 - 1 levels in the 32-bit field, so every `q` is an integer an f32 holds exactly and the CPU and the shader form the same `f32(q)`) and the range rounded outwards so no height is clamped, the height error SHALL be at most half a step plus f32 rounding of the sum (the step is at most 1 mm up to 1 mm x 16,777,215 = 16.7 km of relief in one tile; the f32 sum `hr_min + f32(q) * hr_step` adds about 2^-24 of the relief and of `|hr_min|`, so 1 mm of reconstruction error holds up to roughly 8 km of relief; the field stays 32 bits wide), and every tile SHALL be built compact whatever its relief (ADR 0062: there is no vertex fallback; the wide texel holds CON-13's 1 mm on steep finest tiles, SPIKE-12). The bound of the wide texel is the relief of one tile: the step is relief / 16,777,215, so it is at most 1 mm up to 16.7 km of relief in one tile and grows in proportion above (about 1.19 mm at 20 km, 2.4 mm at 40 km). The builder SHALL NOT refuse a tile for its relief: no spec carries a level-relevant tolerance that a larger step would break (the highest relief of a tile in the specs is the steep stress field's 1,875 m at level 9, and the planet's total relief is below 20 km), so the bound is documented and tested (16.7 km and 20 km), not enforced. Tiles of levels 0-2 are not drawn (LOD-011) and not stored (STRM-021). The CPU mirror of the shader's vertex stage (position, morph target, skirts, heights, normal) SHALL agree with the canonical CPU mesh at every vertex of tiles of levels 3 to 26 within the model error, the quantisation step and f32 rounding, with the exact morph target (neighbour heights read from the texels), and neighbouring tiles of one level SHALL meet within the model error of the two (millimetres from level 12). Tiles SHALL be bit-identical across runs and thread counts, with known-answer hashes next to the mesh version they belong to. Every value in a tile SHALL be tile-local or a unit vector (CON-11).

Verify: A, B, C
Status: active
Source: ADR 0033 points 4 and 5, SPIKE-06, CON-11, CON-13, CON-14, LOD-010, GEN-025, ADR 0062, SPIKE-12

#### Scenario: Layout and bytes
- **WHEN** a tile is built at cells 32 and cells 64
- **THEN** it has 1,089 / 4,225 texels of 8 bytes, a 128-byte header and a 324-byte climate block (slots of 9,168 B and 34,256 B, GEN-025), no texel carries climate, and its key equals the mesh key of the same tile under `MESH_VERSION` 8

#### Scenario: Normals
- **WHEN** 200,000 random directions, the axes and the octahedron's fold are encoded and decoded, and the stored normals of tiles of levels 3 to 22 are compared with the CPU mesh
- **THEN** the largest angle is at most 0.01 degrees (measured 0.0072 with 15 bits)

#### Scenario: Height step
- **WHEN** tiles of the eroded field at levels 3 to 26 are built
- **THEN** every texel is within half a step (plus f32 rounding) of the exact height, no level is clamped, and the step is at most 1 mm at every level for tiles up to 16.7 km of relief

#### Scenario: Format decision
- **WHEN** a finest tile of the steep stress field with more than 65.535 m of relief, a finest tile of the reference field and a coarse tile with large relief are built
- **THEN** all are compact tiles (there is no other form to decide on, ADR 0062), equal to the canonical mesh through the CPU mirror within the model error

#### Scenario: Format follows the cap
- **WHEN** the same steep tile is built with its level as the finest level and with a deeper cap
- **THEN** both are the same compact tile with the same key

#### Scenario: The relief bound of the wide texel
- **WHEN** a tile over a ramp field of 1,000, 16,700, 20,000 and 40,000 m of relief is built
- **THEN** it is built, not refused, and its height step is relief / 16,777,215: 1 mm or less up to 16.7 km, 1.19 mm at 20 km (GEN-019's documented bound)

#### Scenario: Model error and seams
- **WHEN** the CPU mirror of the shader is compared with the canonical mesh over tiles of levels 3 to 26 on the reference, eroded and steep fields, and neighbouring tiles of one level are compared along their shared edge
- **THEN** positions and morph targets are within the biquadratic bound, heights within the quantisation bound, normals within 0.01 degrees, and from level 12 neighbouring tiles meet within 3 mm

#### Scenario: Determinism
- **WHEN** the same tiles are built on 1, 2 and 4 threads
- **THEN** their content hashes are equal to each other and to the committed known answers

#### Scenario: Cliff tiles hold 1 mm
- **WHEN** finest tiles of the steep stress field (1,875 m of relief at level 9, 805 m at level 12, 142 m at level 16, 6.7 m at level 22) are built and compared with the canonical mesh through the CPU mirror
- **THEN** the height step is at most 1 mm and the height error at most 1 mm (f32 granularity of the compared heights) at every level, and the normal error at most 0.01 degrees (CON-13, CON-14; SPIKE-12 measured 0.0071 degrees at level 9 to 12; the step is relief / 16,777,215, 0.1 mm at level 9)

#### Scenario: Image noise of the wide texel
- **WHEN** the ten standard views are drawn from wide tiles and from the previous 16-bit tiles in the relief, normals and height views (recorded once at the change, then the old texel is gone)
- **THEN** at most 1.6 % of a frame's pixels differ, almost all by one level, and at most one pixel per frame by more than 8 levels (SPIKE-12 T10: rounding noise in the sense of ADR 0017)

### Requirement: GEN-020 Batch evaluation is order independent
A height field SHALL offer a batch evaluation of a list of directions that returns, for each direction and in the caller's order, exactly the value `height` returns for it (bit for bit). A field that reads a block-structured lattice source SHALL gather the lattice cells the whole batch reads, read them from the source grouped by block, and evaluate from those cells, so that a tile decodes each block it touches once, whatever the source's cache holds, instead of once per cache miss in raster order. The tile builders (the vertex mesh and the compact tile) SHALL obtain every height they need (lattice samples, normal stencil points, the origin) through one batch call, together with the tile's climate (GEN-021: `heights_and_climate` gathers the cells of both in one pass, so a tile still decodes each block once however small the cache), and the tile SHALL be bit-identical in raster order and in block order, at any thread count. The batch SHALL poll its stop flag before each block group, every 64 directions of the footprint pass, every 64 cells of the gather and every 64 samples and read nothing once it is set. The batch SHALL also be available on up to `threads` threads (`heights_threaded`): the distinct blocks are dealt to the threads, each thread reading the cells of one block at a time, and the evaluation from the gathered cells is split over the threads; the values and the number of block reads SHALL NOT depend on `threads`, and a batch of two directions or more is gathered. No key, bake format, mesh version or golden changes.

Verify: A
Status: active
Source: opus-planner decision of 2026-10-15 (delegation), STRM-014, STRM-005, GEN-017, CON-14, M4 fix F3 (LOD-009)

#### Scenario: Bit identity
- **WHEN** a tile (a level-0 root, a coarse tile spanning many blocks, a fine tile, a tile on a face edge, one on a cube corner) is built in raster order and in block order, over the eroded field on an in-memory bake and on a source with a small cache (1 and 20 threads), and on a stored multi-block bake with a cache of one block against an unbounded one (the store at 1, 4 and 20 threads is also STRM-014's; tier A runs 1 and 3 threads, the full sweep is tier B, `oracle_concurrent_source`)
- **THEN** every vertex bit and the mesh key are equal

#### Scenario: Decodes against distinct blocks
- **WHEN** a coarse tile spanning many blocks is built over a source with a cache of a few blocks
- **THEN** the decodes are at most the distinct blocks the tile touches (also with a cache of one block), raster order many times that

#### Scenario: Cancellation
- **WHEN** the stop flag becomes set during a batch
- **THEN** the batch stops within 64 samples and reads nothing more

#### Scenario: Threaded batch
- **WHEN** a batch of 3, 52 and 300 directions is evaluated on 0, 1, 2, 5 and 20 threads
- **THEN** every value is bit for bit `height` of its direction and every distinct block is read exactly once; a set stop flag reads nothing, and a flag set during the reads stops every thread within 64 reads of its own

### Requirement: GEN-021 Climate field of a tile
A tile SHALL carry its climate (ADR 0039 point 2): the temperature in degrees C and the precipitation in mm per year at 9 x 9 points, texel `(a, b)` at tile sample `(a, b)` of a tile grid of 8 cells, two f16 values per texel, 324 bytes per tile, (the compact tile's climate block in its slot, GEN-025). It is a **field** (ADR 0035 point 1): it is produced by a generator, stored and keyed with the tile and read by the renderer, so it has its own implementation id (`cpu-climate-v1`) and version, and one implementation per cache with the CPU canonical (CON-07). Its values come from the global bake read with the integer reader of ADR 0027 (`read_tile_sample`, by every field over a bake: the terrain field and the eroded field; a wrapper of a field, such as the streamer's cancellable one, delegates `climate`, `climate_version_word` and `heights_and_climate` so a tile built through it has the key and the climate of the inner field; lattice position from integers, face-edge-symmetric interpolation, `READER_VERSION`); a field without a bake (the reference generator, synthetic fields) takes the analytic global-coarse function of its seed at the same directions, a pure function of `(seed, mapping, tile)`. Both producers measure latitude in COORD's axes: +z is the north pole, so at +z the climate is that of latitude 90, at -z that of latitude -90 and at +x and +y that of the equator (the analytic function once took the latitude from the y component, which put its poles at +-y; fixed with `GLOBAL_VERSION` 3, whose bump re-keys every tile through the version word). The stored value SHALL be the f16 rounding (round to nearest, ties to even) of the reader's f32 value, computed with integer arithmetic and `libm` only, so the CPU result is the definition of the field, the implementation id `cpu-climate-v1` is part of the version word, the GPU decode (`unpack2x16float`) is exact and CON-14's CPU/GPU bound for this field is zero; against the bake the rounding costs at most 2^-11 of the value (declared quantisation: half a step, at most 0.008 degrees below 16 C and 0.016 below 32 C, 2 mm of precipitation below 2,048 mm and 4 mm below 4,096 mm; the field's range is 0 to 4,500 mm), a deviation from ADR 0039's "abs 0.01 degrees and 0.1 mm" that f16 cannot meet for large values, recorded here. The version word that enters every mesh key and persistent mesh key (`PARAM_CLIMATE_VERSION`) SHALL cover the climate version, `READER_VERSION` and `GLOBAL_VERSION`. Texels SHALL be bit-identical across runs, thread counts and operating systems, with known answers, neighbouring tiles SHALL share their border texels bit for bit (also across face edges), and an unreadable block SHALL be an error naming it (recorded like a height's, never a default).

Verify: A, B
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

### Requirement: GEN-022 Ocean connectivity in the global bake
The global stage SHALL provide an **ocean-connectivity channel** (ADR 0041 point 2): one bit per vertex of the bake, set where the vertex belongs to the ocean. A vertex is **sea** when its stored continentalness satisfies `f64::from(continent) <= SEA_LEVEL_C` (the complement of the land rule); the sea vertices form a graph under 4-connectivity on the six cube-face grids in which the copies of a vertex on a shared face edge are **one node** (two copies on an edge, three at each of the 8 cube corners, found by exact integer cube points, never by floating point); its connected components are labelled by their smallest node id (a node's id is the smallest raw vertex id `face (res + 1)^2 + j (res + 1) + i` among its copies; future lake ids derive from the label); the **area** of a component is the sum over the copies of its nodes of the solid angle of the dual cell of the vertex clipped to its face (a half cell on an edge, a quarter at a corner; the closed form of a rectangle of the tangent plane, `libm::atan`), in fixed point at 2^-48 of the unit sphere, so that sums are integers and the six faces add up to the sphere to the rounding of each vertex; a component is **ocean** when its area reaches the **area threshold**, a fraction of the body's surface that is body data (`body.<n>.sea.min_area_fraction`, WDEF-006; the Earth default is 1e6 km^2, about 0.196 %), truncated to fixed point; every other sea component, an inland basin below sea level, is dry. The components SHALL be found by a **tiled union-find with one tile per bake block, during the bake**, never by recursion: each block is analysed while it is in memory (a scanline union-find with minimal roots; the nodes and the area of the vertices the block owns, a partition of every face in which a shared border vertex is counted once; the unions across the block's border as one record per maximal run of sea along a side and one per sea corner), the reports are merged in a global union-find over node ids with minimal roots, and a second pass labels each block again and sets the bits. The result (the table of components with label, nodes, area and ocean flag, and every bit) SHALL be bit-identical for every thread count, block size, block order and operating system (CON-14: a set function of the bake, integer sums, IEEE `+ - * /`, `sqrt` and `libm::atan`), and equal to a serial explicit-queue fill over the node graph (the CPU reference, which shares only the vertex-area formula, itself tested against the sphere and against the geometric area of a mid-face cell). Memory SHALL be bounded by the bake's batch: about 10 bytes per vertex beyond a block's samples while it is analysed, and one record per local component and per border run in the merger. Class: **field** (categorical, one bit per vertex, never interpolated); bound: exact, no tolerance (CPU only; there is no GPU twin). A bake with this channel is `GLOBAL_VERSION` 4; its known answers (the table hash and the channel hash of a small bake) SHALL be pinned and equal on every platform and in the determinism manifest (GEN-003). The area formula, the sea rule and the threshold are part of the version: a change of any of them bumps it.

Verify: A
Status: active
Source: ADR 0041 point 2, SPIKE-08, ADR 0026, ADR 0027, ADR 0035, CON-07, CON-14

#### Scenario: Against the serial reference
- **WHEN** real bakes (seeds 1 and 7 at 64 and 128 cells) and random site-percolation masks (res 1 to 32, 55 to 70 % sea, shared nodes equal) are computed tiled with blocks of 1 to the whole face and 1, 2, 3 or 4 threads, at thresholds 0, small, the Earth default, large and 1 (tier A: a slice of this grid (one seed at 64 cells, four of the seven mask sizes with two seeds), threads 1 to 3; the full sweep is tier B, `oracle_ocean_sweep`)
- **THEN** the components (label, nodes, area, ocean flag) and every bit equal the serial reference's, and the block reports can be merged in any order with the same table

#### Scenario: Nodes and areas
- **WHEN** the copies of every vertex of grids of 1 to 16 cells are enumerated, and the vertex areas of grids of 1 to 64 cells are summed
- **THEN** every copy names the same node (the smallest raw id), a grid has `6 (res - 1)^2 + 12 (res - 1) + 8` nodes with 12 (res - 1) edge nodes of two copies and 8 corner nodes of three, and the six faces add up to the sphere to the rounding of each vertex

#### Scenario: A dry basin one vertex from the ocean
- **WHEN** a basin below sea level sits behind a wall of land one vertex wide next to the open sea, with a threshold between them
- **THEN** the sea is one ocean component whose bits reach across the face edge to the copies on the neighbouring face, the basin and a one-vertex pond are separate components and stay dry, and the wall is not ocean

#### Scenario: Known answers
- **WHEN** the 128-cell bake of seed 1 is computed at the Earth threshold
- **THEN** its table has 5 components and its table hash and channel hash equal the pinned values on every platform

### Requirement: GEN-023 The water-level field W
The generators SHALL provide the water-level field W(dir) (ADR 0041 point 1): at a direction, either no water or a **sample** of the surface height above the reference surface in metres (`f64`), the **water-body id** (`u64`; the sea is `WaterBodyId::SEA`, ids with the top bit set are reserved for ids derived from the labels of the bake's components) and the **liquid kind** (`u32`, a row of the world's liquid table, WDEF-006). W SHALL be made by **producers** listed in a `WaterField` over a face mapping and the bake (a `LatticeSource`), most specific first, the first that reports water at a point winning; a producer is a pure function of the query (the direction, its position in the bake's lattice from `locate_direction` and the bake interpolated there by `read_direction`) and its own data. The only producer of M5 is the **sea**: a point is sea when the interpolated continentalness is at or below `SEA_LEVEL_C` **and** at least one of the lattice vertices with a non-zero weight in ADR 0027 point 2 (the lower corner always, the others when their axis weight is not zero) is ocean-connected (GEN-022); the channel is categorical and is not interpolated; W is the sea level of the definition (`body.<n>.sea.level_m`, constant in M5), the kind the definition's sea liquid and the body `SEA`; an inland basin below sea level and land have no water. The coast is defined by the continentalness alone, whatever the level: M5 seas are at the Earth default 0, and a sea level other than 0 is a producer parameter whose coast the water pass (item 7) decides. The set of vertices SHALL be the same nodes from either face of a shared face edge (the rule is edge-symmetric), an ocean channel of another resolution than the bake SHALL be an error naming both, and a missing or damaged block of the bake or of the channel SHALL be an error naming it, never wet or dry. `plan_water(dir)` SHALL give the GPU-free plan data of a view: the level and the body and kind of the water at the direction, else the reference level of the first producer that has one (the sea's) else 0. Class: all attributes are **fields** (CON-07, ADR 0035): the level, the body id and the kind are CPU values, bit-exact for every thread count and operating system, with no GPU twin; the level of the sea equals the definition's value bit for bit. The field's version (`WATER_VERSION`) and implementation id (`cpu-water-level-v1`) are recorded for the keys of whatever stores W later; nothing stores it in M5. The **retrofit** (ADR 0041 point 7): a synthetic lake (a basin with a fixed level inside a footprint, its own body id and a lava kind with emission) from a test-only producer SHALL go through the same `WaterField` path with no change to the sea producer: its water has its own level, body and kind, the sea's answers are the same with and without it, it never replaces sea, and the plan data follows it.

Verify: A
Status: active
Source: ADR 0041 points 1 and 7, ADR 0027 points 2 and 5, ADR 0035, owner's M5 constraints 1, 2, 3 and 5, CON-07, CON-14

#### Scenario: The sea
- **WHEN** 6,000 directions of the 128-cell bake of seed 1 are queried, and the vertices themselves
- **THEN** water is found only where the interpolated continentalness is at or below the sea level and a weighted vertex is ocean, with the sea's level, body and kind, ocean vertices are wet and land vertices dry, and the answers are the same on 1, 2 and 7 threads

#### Scenario: A dry basin
- **WHEN** a basin below sea level lies one vertex behind a wall of land, and points are queried between the ocean vertex and the wall, between the wall and the basin, and in the basin
- **THEN** the first is water (an ocean vertex carries weight), the others are dry although their continentalness is at or below sea level

#### Scenario: Edge symmetry
- **WHEN** a point on a shared face edge is expressed as a lattice position of either face
- **THEN** its weighted vertices are the same nodes

#### Scenario: Synthetic lake
- **WHEN** a lake producer with level L, body 2 and the lava kind is listed before the sea
- **THEN** its centre is water at L with body 2 and kind 1 (emitting), the same directions are dry without it, the sea's answers elsewhere are unchanged, and the plan water over the lake is its level and over dry land the sea level

#### Scenario: Over the store
- **WHEN** the field reads a stored bake and its stored ocean channel (STRM-023)
- **THEN** it gives the answers of the in-memory computation, with the sea level and liquid of the definition, and an ocean channel of another resolution is an error naming both

### Requirement: GEN-024 The coarse level of the water field W
The generators SHALL provide a coarse level of W (ADR 0058), the `WaterSummary` of a bake of `res` cells per face edge in blocks of `B` cells: one cell per `d x d` bake cells, `d = min(B, 64)`, so `6 (res / d)^2` cells (face, row, column, x fastest), each inside one block. A cell holds two flags and a number, functions of the bake and its ocean channel alone (the sea level and the liquid of the definition are no input):
- WET: some lattice cell of the summary cell has an ocean vertex among its four corners and a corner whose continentalness is not above `SEA_LEVEL_C` (a NaN counts as not above), the necessary condition for any point of that lattice cell to be sea (GEN-023);
- OCEAN: some vertex of the summary cell is ocean;
- band elevation: the lowest corner elevation among the lattice cells that have a corner above `SEA_LEVEL_C`, infinity when none has, the necessary condition for a band point (ATMO-019).

`summarize_block` SHALL compute the cells of one block from its samples and ocean bits, and a summary SHALL be the same whatever the order of the blocks, the thread count or the block size `B` (beyond `d`). A tile at or above the bake's block level `log2(res / B)` (a tile whose area is at least one block's) SHALL be answered by `WaterSummary::tile_water`; a tile of a finer level is not answered (`None`: the field is asked point by point, GEN-023). The answer is the sea (the definition's level, the body `SEA`, its liquid) when any cell the tile's lattice rectangle touches, widened by one lattice cell on the low side (a border sample can lie a rounding error below the tile's edge and weigh the vertices there), is WET; else, when any such cell has a band elevation at most `B(W)` (ATMO-019) and a cell with the OCEAN flag has its centre within `100 km + 0.82` of the tile's edge `+ 1` summary cell of the tile's centre (0.82 is above `2/sqrt(6)`, the least with which a parent's reach contains a child's at every level, including beside a cube corner) (the angle counted on the sphere, one `libm` cosine); else nothing. A `SeaProducer` with a summary SHALL answer for the tiles of its summary, a `WaterField` answers when every producer does, and a summary of another resolution than the ocean channel SHALL be an error naming both. Class (ADR 0035): a **field** (CON-07) with no GPU twin; bound: exact, bit-exact for every thread count and operating system (CON-14); a coarse answer is a superset of the point query's, so it never misses water the 25 samples find, and a parent is wet wherever its children are (ATMO-019). Known limit: a sample that the direction mapping places on the neighbouring face of a face edge can weigh a vertex of that face that no cell of the tile's face holds; the ocean channel's edge connectivity covers it except for a NaN continentalness. A summary with more than `6 * 1024^2` cells SHALL NOT be made (refused before allocating).

Verify: A
Status: active
Source: ADR 0058, ADR 0041 point 2, ADR 0056, GEN-022, GEN-023, ATMO-019, CON-07, CON-14

#### Scenario: A hand-made block
- **WHEN** a block of 2 cells is summarised with no ocean vertex, with an ocean vertex among land corners, with an ocean vertex beside a sea corner, with the sea corner in another lattice cell, and with a NaN corner, and a block of 128 cells with one ocean vertex and one sea corner
- **THEN** the flags and band elevations are the documented ones, the 128-cell block gives 2 x 2 cells and the cell holding column 5 and row 100 is WET and OCEAN

#### Scenario: Known tiles
- **WHEN** the six faces of a one-cell-per-face summary are asked, with sea on one, band on two and a high cell on a fourth
- **THEN** the sea face is wet, the band face next to it is wet by the reach, the band face opposite it is dry, the high cell is dry, a face with no cell is dry, the band rises with the sea level, the sea is at the definition's level, and a tile below the block level is not answered

#### Scenario: A real bake
- **WHEN** the summary of a bake of 256 cells is made in blocks of 32, 64 and 256 cells on one and on four threads, from blocks in order and in reverse, and compared with a summary read off the whole faces cell by cell
- **THEN** all agree bit for bit, and the summaries of the 64 and 256 blocks (both with `d` 64) are equal

### Requirement: GEN-025 Compact slot layout
A compact tile SHALL be one slot of contiguous bytes, the same bytes in the CPU tile, the pack payload (STRM-021), the block of the pool (REND-016) and the shader's reads, so no second resource per tile exists (ADR 0062 point 2). At cells 32 the slot is 9,168 bytes, a multiple of 16:

| offset | bytes | content |
|---|---|---|
| 0 | 128 | header: nine control points (108 B), `hr_min`, `hr_step`, origin height, skirt depth (16 B), flags (4 B) |
| 128 | 324 | climate block (GEN-021): 81 words, temperature f16 in the low half, precipitation f16 in the high half |
| 452 | 4 | pad |
| 456 | 8 (cells + 1)^2 | texels (GEN-019), row-major, 8,712 B at cells 32 |

The stride SHALL be `456 + 8 (cells + 1)^2` bytes (9,168 B at cells 32, 2,768 B at cells 16, 34,256 B at cells 64: a multiple of 16 for every even cell count), and the offsets SHALL be constants of the layout version; a later per-tile block is appended after the climate block and moves the texel offset by its size without changing any other offset. `MESH_VERSION` SHALL be 8: every key and persistent key of a compact tile changes, the pack payload version is 3 (STRM-021), and the known-answer hashes of GEN-019 and GEN-021 are re-recorded. The slot SHALL be bit-identical across runs, thread counts and operating systems, and every value in it SHALL be tile-local or a unit vector (CON-11).

Verify: A, B
Status: active
Source: ADR 0062 point 2, ADR 0063, SPIKE-12 Q3, ADR 0033 points 3-4, GEN-019, GEN-021, CON-11, CON-14

#### Scenario: Offsets and stride
- **WHEN** a slot is built at cells 16, 32 and 64
- **THEN** the strides are 2,768, 9,168 and 34,256 bytes, every stride is a multiple of 16, and the header, climate block, pad and texels sit at the offsets of the table

#### Scenario: One byte string
- **WHEN** a tile's slot is encoded to the pack payload, decoded, uploaded to a block and read back
- **THEN** the four byte strings are equal and equal the CPU builder's, and the shader's reads of header, climate and texel words at the offsets agree with the CPU mirror

#### Scenario: Determinism and version
- **WHEN** the slots of 24 tiles are built on 1, 2 and 4 threads
- **THEN** the content hashes are equal to each other and to the committed known answers of `MESH_VERSION` 8
