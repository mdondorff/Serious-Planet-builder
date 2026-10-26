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
A determinism manifest SHALL list output hashes for a committed set of tiles (height and mesh, every face, cube corners, levels 0 to 28) and reference-surface round trips, plus the generator, hash, mesh and key-format versions and the height and mesh keys of the same tiles for the planet and for a moon built through the generator. It records the mesh version, so a change of it (9 since ADR 0073) is a reviewed change of the committed manifest. It SHALL also list the global bake at 64 cells per face edge (the whole-grid hash and the hash of every block of 16 cells at level 2), the raw `f64` height of the terrain field (`cpu-terrain-v1`) and of the eroded field (`cpu-erosion-v1`) at twelve face-coordinate points on every face (the four cube corners, the four edge midpoints, the centre and three further points on and near edges and corners), the component table and the channel of the ocean connectivity of that bake (`oceantable`, `oceangrid`, GEN-022), the versions and implementation ids of the bake and both fields, and the persistent known answers of ADR 0018 (keys, node, body and query hashes, the bake's stage and block keys, an ocean block's key and the node set id; written by `app`, ADR 0023 point 9). A one-ulp change in the value behind any of the new entries SHALL change that entry. CI SHALL write it on Windows and Linux and fail when the two differ, naming every differing entry.

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
A tile mesh SHALL carry a key covering everything its vertices depend on (tile height key, grid size, the body parameter hash with the radius, skirt fraction and a mesh version), and the mesh version SHALL be bumped whenever the CPU mesh output changes for the same inputs, the compact construction (GEN-019) changes or the key composition changes; version 5 introduced the compact tile, which carries the mesh key of its tile; version 6 added the finest-level class (LOD-010: a tile at or deeper than the view's finest level had its key rehashed with the class, because the format of a finest tile was decided from its relief), which `remove-vertex-render-path` removes without a version bump (ADR 0062: no decision depends on the relief any more, so a tile's key is its base key at every level; the class key only named in-memory tiles at the cap, and the persistent pack never stores a tile at or above the cap, STRM-021, so no stored record can answer to a key whose meaning changed); version 7 adds the climate field (GEN-021): a tile carries 9 x 9 f16 texels of temperature and precipitation, and the key names the climate version word (the climate, reader and bake versions) so a change of any of them re-keys every tile, while the vertex output and the compact geometry are unchanged (the committed mesh hashes, which do not cover the climate, stay as they were under version 6); version 8 changes the compact tile to the slot of GEN-025 (wide texel, climate block and record inside the slot): every key changes (the version is a key input), the content of the CPU meshes and the committed mesh hashes stay as they were; version 9 (ADR 0073) defines the canonical normal as the lattice normal (GEN-019): every key changes, the vertex heights and positions are unchanged and the committed mesh hashes are re-recorded. The plan's keys and the compact tiles of one view SHALL agree: a tile has one key whatever the finest level of the view (the same tile under another cap is the same tile with the same key). Persisted height tiles (STRM-009) are not meshes and are unchanged by it; persistent mesh keys (kind mesh) change with the version, and repro bundles of an older mesh version are refused with the versions named (TEST-008). Known-answer hashes of committed meshes SHALL be identical on every platform and SHALL be asserted next to the version they belong to.

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
The tile builder SHALL produce, for levels 3 and finer, a compact tile (ADR 0033 point 4): one 8-byte texel per grid vertex (`(cells + 1)^2` texels: a 32-bit height step count between the tile's own minimum and maximum, then in the second word two signed 15-bit octahedral normal components in bits 0-14 and 15-29, bits 30-31 zero), the tile's climate block (GEN-021: 81 words, 324 bytes), its driver block (GEN-074: 81 words, 324 bytes) and a 128-byte header (nine tile-local control points of a biquadratic base surface on the shell through the tile origin, the height range `hr_min` and `hr_step`, the origin height, the skirt depth and the flags: bit 0 the orientation flip, every other bit zero), laid out as one slot of 9,488 bytes at cells 32 (GEN-025; `MESH_VERSION` 10). A compact tile keeps the minimum of 16 cells per tile edge (a build with fewer cells is refused naming the minimum; the former reason, 162 climate values in 162 texels, is gone, the minimum stays so that the cells 16 configurations and their pool arithmetic are unchanged). A tile's key SHALL be the mesh key of its tile (GEN-010). The canonical normal of a vertex (ADR 0073) SHALL be the cross product of the centred differences of the positions of its lattice neighbours, `n = (q(k+1, l) - q(k-1, l)) x (q(k, l+1) - q(k, l-1))` normalised with `q = d (R + h)` in f64, where `d` is the neighbour's unit direction and `h` its height; every face has `U x V` outward, so `n` is outward without a flip, and a flip is an assertion failure (except at the eight cube corners, below). The neighbours are the tile's own lattice points and a one-sample ring around them (without its four corners); a ring point across a tile edge inside a face is the next tile's lattice point, and one beyond a face edge is the neighbour tile's own lattice direction one cell inside it, on the row that continues the shared edge, found from the exact integer cube point and never by projecting the extended plane back (`TileId::ring_direction`). Two tiles sharing a vertex therefore read the same directions and compute bit-identical normals, inside a face and across the twelve cube edges (a cross product negates exactly when both operands or their order change sign). At a cube corner the three tiles read the same three edge points, order them canonically by the pair of faces that hold each (`TileId::corner_stencil`), form `(p1 - p0) x (p2 - p0)` from the same operands and orient it outward; their normals are bit-identical. The CPU mesh and the compact tile use this one definition. The stored normal SHALL equal the canonical CPU normal within 0.01 degrees (CON-14); the height SHALL be reconstructed as `hr_min + f32(q) * hr_step` with the step the relief over 16,777,215 (2^24 - 1 levels in the 32-bit field, so every `q` is an integer an f32 holds exactly and the CPU and the shader form the same `f32(q)`) and the range rounded outwards so no height is clamped, the height error SHALL be at most half a step plus f32 rounding of the sum (the step is at most 1 mm up to 1 mm x 16,777,215 = 16.7 km of relief in one tile; the f32 sum `hr_min + f32(q) * hr_step` adds about 2^-24 of the relief and of `|hr_min|`, so 1 mm of reconstruction error holds up to roughly 8 km of relief; the field stays 32 bits wide), and every tile SHALL be built compact whatever its relief (ADR 0062: there is no vertex fallback; the wide texel holds CON-13's 1 mm on steep finest tiles, SPIKE-12). The bound of the wide texel is the relief of one tile: the step is relief / 16,777,215, so it is at most 1 mm up to 16.7 km of relief in one tile and grows in proportion above (about 1.19 mm at 20 km, 2.4 mm at 40 km). The builder SHALL NOT refuse a tile for its relief: no spec carries a level-relevant tolerance that a larger step would break, so the bound is documented and tested (16.7 km and 20 km), not enforced. Tiles of levels 0-2 are not drawn (LOD-011) and not stored (STRM-021). The CPU mirror of the shader's vertex stage (position, morph target, skirts, heights, normal) SHALL agree with the canonical CPU mesh at every vertex of tiles of levels 3 to 26 within the model error, the quantisation step and f32 rounding, with the exact morph target (neighbour heights read from the texels), and neighbouring tiles of one level SHALL meet within the model error of the two (millimetres from level 12). Tiles SHALL be bit-identical across runs and thread counts, with known-answer hashes next to the mesh version they belong to. Every value in a tile SHALL be tile-local or a unit vector (CON-11).

Verify: A, B, C
Status: active
Source: ADR 0033 points 4 and 5, SPIKE-06, CON-11, CON-13, CON-14, LOD-010, GEN-025, ADR 0062, SPIKE-12

#### Scenario: Layout and bytes
- **WHEN** a tile is built at cells 32 and cells 64
- **THEN** it has 1,089 / 4,225 texels of 8 bytes, a 128-byte header and a 324-byte climate block and a 324-byte driver block (slots of 9,488 B and 34,576 B, GEN-025), no texel carries climate, and its key equals the mesh key of the same tile under `MESH_VERSION` 10

#### Scenario: Normals
- **WHEN** 200,000 random directions, the axes and the octahedron's fold are encoded and decoded, and the stored normals of tiles of levels 3 to 22 are compared with the CPU mesh
- **THEN** the largest angle is at most 0.01 degrees (measured 0.0072 with 15 bits)

#### Scenario: The lattice normal
- **WHEN** the lattice normals of tiles over a smooth wave of 20 km are compared with a converged normal (centred differences of 1 m in a tangent frame), inside a face and at the face edges, and a flat field is drawn
- **THEN** inside a face they agree within 0.0002 degrees, at a face edge (the ring point comes from the neighbour face) and at a cube corner within 0.015 degrees, and a sphere's normals are radial within 0.01 degrees

#### Scenario: Seams
- **WHEN** the tiles at the four corners of every face, and tiles on every face edge with their four neighbours, at levels 3, 12 and 22, are built on one and on three threads over the eroded and the steep field
- **THEN** every pair of vertices with the same direction bits, inside a face, across the twelve cube edges and at the eight cube corners (three tiles each), has the same compact word, the same CPU mesh normal and the same f64 normal bit for bit, and the thread count changes nothing

#### Scenario: The ring
- **WHEN** the ring points of tiles at face edges are compared with the neighbour tile's lattice, and the stencils of the corner tiles at the eight corners at levels 3, 12 and 22 are compared
- **THEN** every ring point beyond a face edge is a lattice point of the neighbour inside it and not on the shared edge, and the three tiles at a corner list the same three points in the same order

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
A height field SHALL offer a batch evaluation of a list of directions that returns, for each direction and in the caller's order, exactly the value `height` returns for it (bit for bit). A field that reads a block-structured lattice source SHALL gather the lattice cells the whole batch reads, read them from the source grouped by block, and evaluate from those cells, so that a tile decodes each block it touches once, whatever the source's cache holds, instead of once per cache miss in raster order. The tile builders (the vertex mesh and the compact tile) SHALL obtain every height they need (lattice samples, the one-sample ring around them without its four corners, the origin: 1,222 directions at 32 cells against the 5,446 of the former four-point stencil, ADR 0073) through one batch call, together with the tile's climate (GEN-021: `heights_and_climate` gathers the cells of both in one pass, so a tile still decodes each block once however small the cache), and the tile SHALL be bit-identical in raster order and in block order, at any thread count. The batch SHALL poll its stop flag before each block group, every 64 directions of the footprint pass, every 64 cells of the gather and every 64 samples and read nothing once it is set. The batch SHALL also be available on up to `threads` threads (`heights_threaded`): the distinct blocks are dealt to the threads, each thread reading the cells of one block at a time, and the evaluation from the gathered cells is split over the threads; the values and the number of block reads SHALL NOT depend on `threads`, and a batch of two directions or more is gathered. No key, bake format, mesh version or golden changes by the batch itself (ADR 0073 changes the points it asks for).

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
The generators SHALL provide the water-level field W(dir) (ADR 0041 point 1): at a direction, either no water or a **sample** of the surface height above the reference surface in metres (`f64`), the **water-body id** (`u64`; the sea is `WaterBodyId::SEA` (= 1), ids with the top bit set are reserved for ids derived from the labels of the bake's components) and the **liquid kind** (`u32`, a row of the world's liquid table, WDEF-006). W SHALL be made by **producers** listed in a `WaterField` over a face mapping and the bake (a `LatticeSource`), most specific first, the first that reports water at a point winning; a producer is a pure function of the query (the direction, its position in the bake's lattice from `locate_direction` and the bake interpolated there by `read_direction`) and its own data. The only producer of M5 is the **sea**: a point is sea when the interpolated continentalness is at or below `SEA_LEVEL_C` **and** at least one of the lattice vertices with a non-zero weight in ADR 0027 point 2 (the lower corner always, the others when their axis weight is not zero) is **wet** (ADR 0071): an ocean vertex (GEN-022) or one whose wet floor (GEN-031) is at most `bound_code(B(W))`, the conduction bound of ATMO-019 at the sea's level (a `SeaProducer` without a floor lattice has the ocean vertices only); the channels are categorical and are not interpolated, and `wet_at` answers the vertex rule alone (a wet vertex weighs in the point), which the coastal band reads (ATMO-019); W is the sea level of the definition (`body.<n>.sea.level_m`, constant in M5), the kind the definition's sea liquid and the body `SEA`; an inland basin below sea level and land have no water. The coast is defined by the continentalness alone, whatever the level: M5 seas are at the Earth default 0, and a sea level other than 0 moves no coast (scenario "Sea level"). The set of vertices SHALL be the same nodes from either face of a shared face edge (the rule is edge-symmetric), an ocean channel or a floor lattice of another resolution than the bake SHALL be an error naming both, and a missing or damaged block of the bake or of the channel SHALL be an error naming it, never wet or dry. `plan_water(dir)` SHALL give the GPU-free plan data of a view: the level and the body and kind of the water at the direction, else the reference level of the first producer that has one (the sea's) else 0. Class: all attributes are **fields** (CON-07, ADR 0035): the level, the body id and the kind are CPU values, bit-exact for every thread count and operating system, with no GPU twin; the level of the sea equals the definition's value bit for bit. The field's version (`WATER_VERSION`) and implementation id (`cpu-water-level-v2`, `WATER_VERSION` 2 since ADR 0071) are recorded for the keys of whatever stores W later; nothing stores it in M5. The **retrofit** (ADR 0041 point 7): a synthetic lake (a basin with a fixed level inside a footprint, its own body id and a lava kind with emission) from a test-only producer SHALL go through the same `WaterField` path with no change to the sea producer: its water has its own level, body and kind, the sea's answers are the same with and without it, it never replaces sea, and the plan data follows it.

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

#### Scenario: Sea level
- **WHEN** the sea level is not 0
- **THEN** no coast of the continentalness test moves; the wet bound B(W) follows the level (ATMO-019), the water pass floods per vertex where `W - h_morph > 0`, so a negative level moves the waterline seawards and a positive level floods the ground that ATMO-019 conducts (ADR 0053 point 4, amended by ADR 0071)

### Requirement: GEN-024 The coarse level of the water field W
The generators SHALL provide a coarse level of W (ADR 0058, ADR 0071), the `WaterSummary` of a bake of `res` cells per face edge in blocks of `B` cells: one cell per `d x d` bake cells, `d = min(B, 64)`, so `6 (res / d)^2` cells (face, row, column, x fastest), each inside one block. A cell holds a flag and two numbers, functions of the bake and its wet floor (GEN-031) alone (the sea level and the liquid of the definition are no input):
- SEA: some lattice cell of the summary cell has a corner whose continentalness is not above `SEA_LEVEL_C` (a NaN counts as not above), the necessary condition for any point of that lattice cell to be sea (GEN-023);
- floor: the lowest wet floor (`u16`, 0 where an ocean vertex is, 0xFFFF where the sea reaches none) over the corners of the lattice cells of the summary cell;
- band elevation: the lowest corner elevation among the lattice cells that have a corner above `SEA_LEVEL_C`, infinity when none has, the necessary condition for a band point (ATMO-019).

`summarize_block` SHALL compute the cells of one block from its samples and its floors, and a summary SHALL be the same whatever the order of the blocks, the thread count or the block size `B` (beyond `d`). A tile at or above the bake's block level `log2(res / B)` (a tile whose area is at least one block's) SHALL be answered by `WaterSummary::tile_water`; a tile of a finer level is not answered (`None`: the field is asked point by point, GEN-023). The answer is the sea (the definition's level, the body `SEA`, its liquid) when any cell the tile's lattice rectangle touches, widened by one lattice cell on the low side (a border sample can lie a rounding error below the tile's edge and weigh the vertices there), has a floor at most `bound_code(B(W))` (ATMO-019) and has the SEA flag or a band elevation at most `B(W)`; else nothing. No distance, face mapping or radius enters the rule. A `SeaProducer` with a summary SHALL answer for the tiles of its summary, a `WaterField` answers when every producer does, and a summary of another resolution than the ocean channel SHALL be an error naming both. Class (ADR 0035): a **field** (CON-07) with no GPU twin; bound: exact, bit-exact for every thread count and operating system (CON-14); a coarse answer is a superset of the point query's, so it never misses water the 25 samples find, and a parent is wet wherever its children are (ATMO-019). Known limit: a sample that the direction mapping places on the neighbouring face of a face edge can weigh a vertex of that face that no cell of the tile's face holds; the floor's edge connectivity (GEN-031) covers it except for a NaN continentalness. A summary with more than `6 * 1024^2` cells SHALL NOT be made (refused before allocating). `SUMMARY_IMPLEMENTATION_ID` is `cpu-water-summary-v2`; the version-1 summary (ocean flags, no floor) is not read.

Verify: A
Status: active
Source: ADR 0058, ADR 0071, ADR 0041 point 2, ADR 0056, GEN-022, GEN-023, GEN-031, ATMO-019, CON-07, CON-14

#### Scenario: A hand-made block
- **WHEN** a block of 2 cells is summarised with no floor reached, with floors among its corners, with a sea corner and with a NaN corner, and a block of 128 cells with one sea corner and one floor
- **THEN** the flags, floors and band elevations are the documented ones (the floor is the lowest over every corner), the 128-cell block gives 2 x 2 cells and the cell holding column 5 and row 100 has SEA and that floor, and a block of the wrong size is refused

#### Scenario: Known tiles
- **WHEN** the six faces of a one-cell-per-face summary are asked, with sea at floor 0 on one, band reached on one, band not reached on one, band behind ground one code above the bound, and a high cell
- **THEN** the sea face and the reached band face are wet, the others are dry whatever their distance, the high cell is wet only at a level whose band covers it, the sea is at the definition's level, and a tile below the block level is not answered

#### Scenario: A real bake
- **WHEN** the summary of a bake of 256 cells is made in blocks of 32, 64 and 256 cells on one and on four threads, from blocks in order and in reverse, and compared with a summary read off the whole faces cell by cell from the serial floors
- **THEN** all agree bit for bit, and the summaries of the 64 and 256 blocks (both with `d` 64) are equal

#### Scenario: Parents
- **WHEN** every tile of the levels from 1 to the block level of a real bake is asked
- **THEN** the parent of every wet tile is wet

### Requirement: GEN-025 Compact slot layout
A compact tile SHALL be one slot of contiguous bytes, the same in the CPU tile, the pack payload (STRM-021), the pool block (REND-016) and the shader's reads (ADR 0062 point 2). The stride SHALL be `776 + 8 (cells + 1)^2` bytes, a multiple of 16 for every even cell count, with these offsets as constants of the layout version, and `MESH_VERSION` SHALL be 10 (payload version 4):

| offset | bytes | content |
|---|---|---|
| 0 | 128 | header: nine control points, `hr_min`, `hr_step`, origin height, skirt depth, flags |
| 128 | 324 | climate block (GEN-021): 81 words, temperature f16 low, precipitation f16 high |
| 452 | 324 | driver block (GEN-074): 81 words, soil depth, moisture, insolation, neutral insolation |
| 776 | 8 (cells + 1)^2 | texels (GEN-019), row-major |

The slot SHALL be bit-identical across runs, thread counts and operating systems, with every value tile-local or a unit vector (CON-11).

Verify: A, B
Status: active
Source: ADR 0062 point 2, ADR 0063, ADR 0076, SPIKE-12 Q3, ADR 0033 points 3-4, GEN-019, GEN-021, GEN-074, CON-11, CON-14

#### Scenario: Offsets and stride
- **WHEN** a slot is built at cells 16, 32 and 64
- **THEN** the strides are 3,088, 9,488 and 34,576 bytes (8,712 B of texels at cells 32), every stride is a multiple of 16, and the header, climate block, driver block and texels sit at the offsets of the table

#### Scenario: One byte string
- **WHEN** a tile's slot is encoded to the pack payload, decoded, uploaded to a block and read back
- **THEN** the four byte strings are equal and equal the CPU builder's, and the shader's reads of header, climate, driver and texel words at the offsets agree with the CPU mirror

#### Scenario: Determinism and version
- **WHEN** the slots of 24 tiles are built on 1, 2 and 4 threads
- **THEN** the content hashes are equal to each other and to the committed known answers of `MESH_VERSION` 10; the driver block moved the texels from byte 456 to 776 and each key and persistent key changes with the version

### Requirement: GEN-026 The stamp stage
A stamp stage SHALL add the stamps of one layer of a world definition to a base height field. A stamp has an anchor, a reach `R` (its `reach_m`, above 0), a bound `M` (its `max_delta_m`), a kind and parameters in metres; with `d` the arc distance from the anchor to the point on the body's reference sphere, computed in f64 with `libm` as `radius * atan2(|a x p|, a . p)`, and `x = d / R`, its delta SHALL be zero for `x >= 1`, C1 at `x = 1`, and never exceed `M` in absolute value. A `hill` SHALL add `amount * (1 - x^2)^2`. A `crater` SHALL add `amount * (-bowl(x) + 0.2 * rim(x))` with `amount` its depth (at least 0), `bowl(x) = 1 - s(min(x / 0.8, 1))`, `rim(x) = (1 - ((x - 0.85) / 0.15)^2)^2` for `0.7 <= x <= 1` and 0 elsewhere, and `s(t) = t^2 (3 - 2t)`. A `plateau` SHALL add `w(x) * clamp(target - h, -M, M)` with `target` an absolute height above the reference sphere, `h` the running height at the point and `w(x) = 1 - s(clamp((x - 0.7) / 0.3, 0, 1))`; it reads nothing at its anchor. Stamps SHALL apply in application order (priority, then id), each to the running height. The stage's height at a point SHALL be the base height plus the stamps whose reach contains it, SHALL depend on nothing but the base heights, the stamps of the layer and the point, and SHALL be identical at any thread count, batch order and batch size (GEN-020); it fetches its items once per batch through a reader, so a point is a batch of one. The stage's `height_bounds` for a cap SHALL be the range of the running height at the points of the cap: the base bounds carried through the items whose reach touches the cap, in application order, in f64 with outward rounding. A hill and the bowl of a crater widen the range by the range of their delta, which is monotone in `x` (taken at the nearest and the farthest distance of the cap from the anchor; the crater's profile rises to the rim's crest at `x = 0.85` and falls beyond it, so its range is that of the two end values and, where the cap holds `x = 0.85`, the crest's). A plateau maps the range: `h + w clamp(target - h, -M, M)` is non-decreasing in `h` and linear in `w`, so the image of the range is taken from the ends of the range and the ends of the weight over the cap (`w` at the farthest and at the nearest distance). Where the weight is 1 over the cap the image is the range of the gap to the target; the declared bound `M` limits it and is never added on both sides. A cap on the flat top of a hill or of a plateau therefore grows by centimetres or by the extent of the base there, not by `M`, and planning finds no empty morph interval where the base found none, whatever the declared bounds of the items. A new stamp kind SHALL have a profile monotone in `x` on each part or a bounds rule of its own. The widening is done in the stage only. The stage SHALL delegate everything else (body, seed, surface, climate, water cull level) to the base, SHALL read the base only as a height field at points, and SHALL declare its items for a tile with the halo `STAGE_HALO` = 0.25 of the tile edge.

Verify: A, B, C
Status: active
Source: ADR 0066 points 1, 4 and 5, ADR 0067, ADR 0021 point 7, SPIKE-13, M6 plan row E2, owner test 2026-10-20 (CRSH-0001 to 0004)

#### Scenario: Profiles
- **WHEN** each kind is evaluated at fixed `x` from 0 to beyond 1
- **THEN** the values equal the pinned known answers, the delta and its slope are within 1e-12 of 0 at `x = 1`, it is exactly 0 beyond the reach, and its absolute value never exceeds `M` on a dense sweep

#### Scenario: Plateau
- **WHEN** a plateau is applied to a ramp
- **THEN** the ground inside `x <= 0.7` is flat to the clamp, the change is at most `M`, the weight is continuous at `x = 0.7`, and nothing changes beyond `x = 1`

#### Scenario: Order
- **WHEN** a hill and a plateau cover the same ground, in each order of priority
- **THEN** the heights differ and each equals its own sequential application

#### Scenario: Determinism
- **WHEN** a tile of a stamped field is generated at 1 and 3 threads, with its points in reversed order, and one point at a time
- **THEN** the heights are bit-identical

#### Scenario: Bounds
- **WHEN** every height of caps over stamps is sampled, a field with a stamp of bound 60 m is planned with tiles down to level 19 under the camera (the stage test), and a steep base with stamps under the camera is planned in the two ground views down to level 18 or finer
- **THEN** every sampled height lies within the stage's bounds, a cap on the top of a hill grows the bounds' extent by less than a tenth of `M`, and the plan finds no empty morph interval

#### Scenario: A plateau of a large declared bound
- **WHEN** a plateau of 15 km reach, a target of 300 m, -2,000 m or 4,000 m and a declared bound of 5 km is bounded over caps of 150 m to 2 km in its flat part and sampled over its edge, and plateaus overlap a hill and a crater over the reference terrain and over a steep base in random caps
- **THEN** a cap in the flat part grows the bounds' extent by less than the base's own extent plus 1 m (not by 10 km), and every sampled height lies within the bounds

#### Scenario: Declared against recorded reads
- **WHEN** the stage generates a tile with a recording reader
- **THEN** every item the reader returned is in the footprint result the tile declared, and the output is unchanged beyond `reach_m` of every stamp and within `max_delta_m`

#### Scenario: Tile alone
- **WHEN** a stamped tile over the eroded field (an in-memory bake) is generated alone and its samples are cut from its parent's grid
- **THEN** they are bit-identical, whatever the order of generation

#### Scenario: Stamped scene
- **WHEN** a stamped coast scene is drawn in the debug relief view on each adapter family
- **THEN** the frame hash equals the one recorded for that family (WARP and the RTX; an adapter with no recorded hash prints its own), the unstamped and the stamped scene differ, and the images are written to `target/review/stamps/` for inspection

### Requirement: GEN-028 The line stage
The stamp stage (GEN-026) SHALL also apply the line items `ridge` and `channel` (WDEF-011) of its layer. Let `d` be the distance in metres, on a sphere of the body's radius, from a point to the polyline of the item: the minimum over its segments of the great-circle distance to the segment (to the arc where the point's foot lies on it, else to the nearer endpoint), computed in f64 with `libm` from unit vectors, and `x = d / width_m`. The delta of a ridge SHALL be `amount_m (1 - x^2)^2` and that of a channel `-amount_m` for `x <= 0.3` and `-amount_m (1 - s((x - 0.3) / 0.7))` beyond, with `s(t) = t^2 (3 - 2t)`, both 0 for `x >= 1`; both are monotone in `x`. The stage SHALL apply the hills, craters and plateaus in application order and then add to the height `max(0, the largest ridge delta)` and then `min(0, the smallest channel delta)` (a delta that is 0 adds nothing, so a base of `-0.0` keeps its sign) over the line items whose reach contains the point, so the result does not depend on the order of the line items and overlapping parts of a line or crossing lines do not double. The delta range of a line item over a cap (the bounds of GEN-026, ADR 0067 point 4) SHALL come from the nearest and farthest distance of the cap to the polyline, `max(0, D - R a)` and `D + R a` for the distance `D` of its centre and its angle `a`, widened outward by the slack of the stamps, and SHALL enclose every delta the item adds at any sampled point of the cap. The stage's pass-through, key chain and footprint declaration are those of the stamps; a tile out of every line's reach is bit-identical to its source.

Verify: A
Status: active
Source: ADR 0069, ADR 0067, GEN-026, M6 plan row E5, owner (2026-10-19)

#### Scenario: Profiles
- **WHEN** the delta of a ridge and of a channel is evaluated from the polyline outward
- **THEN** both are monotone, the ridge is `amount_m` on the polyline and the channel `-amount_m` out to 0.3 of the width, and both are 0 at and beyond the width

#### Scenario: Corners and crossings
- **WHEN** a polyline bends sharply, two lines of one group meet at their end point, in either order, and a ridge crosses a channel
- **THEN** the height at the overlap is the same in both orders, a joint is not higher than one line's crest, and the crossing adds the ridge's and the channel's deltas

#### Scenario: Bounds
- **WHEN** a polyline and the caps around it, near the pole and across the antimeridian included, are sampled densely
- **THEN** every sampled delta lies within the range the bounds give, and the range is not wider than the stamp-sum rule would give

#### Scenario: Contract
- **WHEN** the stage runs on a tile with a recording reader
- **THEN** every line item it touched is in the footprint the tile declared, the output is unchanged beyond `reach_m` of every item and within `max_delta_m`, and a tile away from the lines keys and renders as its source

### Requirement: GEN-029 The region mask
The stamp stage (GEN-026) of a layer SHALL leave the height at a point as the base's when a region of mode 2 ("clip inside" in ADR 0069: the layer's edits are cut inside the outline and stay outside it; keeping them only inside is the mode 1, "clip outside") filed in that layer (WDEF-012) contains the point: every delta of the layer's items (hill, crater, plateau, ridge and channel, GEN-028) is cut there, whatever the order of the items; the edge is hard (a delta is kept or removed per point, never scaled in between). Where no region contains the point the stage is what it was. A region changes no height itself and a region of mode 0 changes nothing. The delta range of a stage over a cap (the bounds of GEN-026, ADR 0067 point 4) SHALL be that of the unmasked stage widened to hold 0 when a region of mode 2 is among the items that meet the cap (a point the region cuts has no delta at all). The pass-through of a tile with no item and no region in its footprint, the key chain, and the output being unchanged beyond `reach_m` of every item and region are those of the stamp stage; a region re-keys the tiles of its layer that its reach touches and no others.

Verify: A
Status: active
Source: ADR 0069, ADR 0067, GEN-026, M6 plan row E5, owner (2026-10-19)

#### Scenario: Inside
- **WHEN** a hill reaching across the edge of a mode-2 region and a ridge crossing it are applied
- **THEN** the height inside the region is the base's, the height outside carries the hill and the ridge, a region of mode 0 changes nothing, and the result does not depend on the order of the items

#### Scenario: Other layers
- **WHEN** a region clips layer A and a hill lies in layer B over the same place
- **THEN** layer B's hill is applied in full

#### Scenario: Bounds
- **WHEN** a mode-2 region overlaps stamps and lines and the caps around them are sampled densely
- **THEN** every sampled masked height lies within the bounds of the stage, and a point inside the region is the base's bit for bit

#### Scenario: Contract
- **WHEN** the stage runs on a tile with a recording reader
- **THEN** every region it touched is in the footprint the tile declared, a tile away from every item and region keys and renders as its source, and editing a region re-keys the tiles of its layer that it touches and no others

### Requirement: GEN-030 The brush stage
The stamp stage of a layer (GEN-026) SHALL apply the strokes of every brush item (WDEF-013) whose reach touches a point, in application order of the items and, within an item, in file order, to the running height `h` at the point's direction `dir` on a body of radius `R`: with `d` the angle between `dir` and the stroke's polyline (the minimum over its great-circle arcs, the angle to the point for a one-point stroke), `x = d R / radius_m` and the weight `w(x) = (1 - x^2)^2` for `x < 1` and 0 otherwise, a raise SHALL add `strength_m w`, a lower SHALL subtract it, a flatten SHALL add `w clamp(target_m - h, -strength_m, strength_m)` and a smooth SHALL add `w clamp((m - b) - s, -strength_m, strength_m)`, where `b` is the height of the procedural base (the field below every authored layer, whose key is field-wide: ADR 0066 point 4) at `dir`, `s` the sum of what the earlier smooth strokes of every brush item of the layer added at `dir`, in application order, so repeated strokes, whether of one item or of the many a session of drags makes, converge on `m` and never pass it, and `m` the mean of that base at the eight directions at the angle `radius_m / (2 R)` from `dir` on the bearings 0, 45, ... 315 degrees from the local east axis (f64, `libm`). The result SHALL be a function of the direction and the items alone: equal at a point two tiles share, and independent of the thread count, the batch and the order the points are asked in (CON-14). A region of mode 2 (GEN-029) that contains the point SHALL cut the brush like every item of its layer. The range of the brush strokes at a cap (GEN-026 bounds) SHALL be carried through the strokes in order as the other items' is: a raise adds `[s w(x_far), s w(x_near)]` and a lower the negative (the weight at the farthest and the nearest distance of the cap from the stroke), a flatten maps the range as a plateau of bound `s` does, and the smooth strokes between two flattens add at most `s w(x_near)` each and at most together the width `max(0, d_hi) - min(0, d_lo)` of the distance `d` of the height to the mean `m`, from the bounds of the procedural base over the cap and over the cap widened by the ring (what the strokes of the layer moved together stays between 0 and `d`); all widened by a few ulps, and the output of the strokes SHALL stay within it. A layer without a brush item SHALL give the output it gave before this requirement, and the stage SHALL read the procedural base only through the height field at points, never a lower authored layer off the tile's footprint, and a stroke or smooth that adds exactly 0 SHALL leave the bits of the running height alone.

Verify: A
Status: active
Source: ADR 0070, M6 plan row E6, GEN-026, GEN-028, owner test 2026-10-20 (brush 23)

#### Scenario: Each kind
- **WHEN** a stroke of each kind is applied over a flat input and over a slope, along and across the stroke, at its end and far from it
- **THEN** a raise adds `strength_m` on the line and 0 at `radius_m`, a lower is its negative, a flatten moves the height towards the target by at most `strength_m` and a second pass moves it further, a smooth moves a peak down and a pit up and leaves a plane unchanged, and every change is at most `strength_m` times the weight

#### Scenario: Order and seams
- **WHEN** an item with a raise then a flatten and the item with them swapped are applied, and the heights of a tile border are computed from the two tiles that share it, with one thread and with four
- **THEN** the two orders differ where they overlap, the shared points agree bit for bit, and the threads change nothing

#### Scenario: Bounds
- **WHEN** the bounds of a brush item at caps from the stroke's centre to far away are compared with the stage's output on a dense sample of each cap, in random caps over raise, lower, smooth and flatten strokes of several items over the reference terrain and a steep base, and with 64 smooth strokes at a point of a base with exact bounds
- **THEN** every output is inside the bounds (rounding of every stroke included), the stage moves the base by at most the sum of the strengths, and 32 smooth items of 40 m strength grow the extent of a fine tile by less than twice the base's extent plus 40 m

#### Scenario: Smooth strokes of several items converge together
- **WHEN** one to eight items each with a smooth stroke of 7 m strength are applied at one point over a peak, and the same strokes as the strokes of one item
- **THEN** the height lies between the ring mean and the peak, never past the mean, and equals the height of the one item bit for bit

#### Scenario: Smooth reads the procedural base and converges
- **WHEN** a smooth stroke is applied over a hill of a lower layer with two different heights, and one to six smooth passes are applied over a peak
- **THEN** what the brush layer adds is the same over both hills, and the height after any number of passes lies between the ring mean and the peak, never past the mean

#### Scenario: Tight bounds
- **WHEN** an item of 64 raise strokes on one line is applied over a field whose bounds are exact, and a ring is built at a pole
- **THEN** the point's height lies inside the bounds of every cap that holds it, the ring is finite, and a delta of exactly 0 leaves a height of -0.0 as it was

### Requirement: GEN-031 The wet floor
The generator SHALL compute for every vertex of the global bake its wet floor (ADR 0071): the minimum over the paths from the ocean to the vertex, on the graph of the 8 neighbours of the cube-sphere lattice (copies of one node on a shared face edge merged), of the maximum code on the path. The code of an ocean vertex SHALL be 0, that of a vertex of finite elevation `e` SHALL be `clamp(floor(2 e) + 24000, 1, 0xFFFE)` (an infinite elevation clamps, a NaN elevation gives 0xFFFF and is never crossed), and the floor of a vertex no path reaches SHALL be 0xFFFF. The code of a bound `b` in metres SHALL be `bound_code(b)`, the same expression rounded down and clamped, with 0 for a NaN bound, so that a vertex of elevation at most `b` has a code at most `bound_code(b)`. The tiled computation SHALL use three passes: per block a union-find that emits the edges at which components of the block merge (the block's border nodes and a virtual ocean node as terminals), one global pass over those edges in weight order that gives the floor of every border node, and per block a bucketed priority flood seeded from the block's ocean vertices and the border floors. The result SHALL equal the serial priority flood over the whole bake bit for bit, for every block size and thread count (CON-14). The channel SHALL be a function of the ocean bits and the elevations alone, with no dependence on the sea level or the ocean area threshold beyond the ocean bits themselves.

Verify: A
Status: active
Source: ADR 0071 point 3, SPIKE-21, GEN-021

#### Scenario: Equal to the serial flood
- **WHEN** random grids of 4 to 32 cells with NaN elevations and ocean vertices, and real bakes of 64 and 128 cells, are computed in blocks of every power of two from 1 and on 1 and 3 threads
- **THEN** every vertex's floor equals the serial reference's

#### Scenario: A ridge decides the floor
- **WHEN** a ridge of 90 m crosses a flat plain of 10 m with the ocean on one side
- **THEN** the vertices on the ocean's side have the code of 10 m, those on the ridge and beyond it the code of 90 m, and a bound of 100 m is above the far side's code, a bound of 64 m below it

#### Scenario: Lagoons, diagonals and pits
- **WHEN** a lagoon lies behind a 40 m barrier joined to the ocean only diagonally, and a pit is enclosed by a ring of NaN elevations
- **THEN** the lagoon's floor is the barrier's code, the pit and its surroundings inside the ring have 0xFFFF, and a NaN is never crossed

#### Scenario: Codes
- **WHEN** elevations of 0, 0.4, 0.5, -0.1, NaN and infinities and bounds from -740 to 740 m are coded
- **THEN** the codes round down, clamp inside 1 to 0xFFFE, keep the sentinels and are monotone, and a vertex at or below a bound has a code at or below the bound's

### Requirement: GEN-027 A stage with nothing to add is the identity
An authored stage SHALL say, for a tile, whether it passes the tile through (`HeightField::passes_through`); it MAY say yes only when its output for every sample of the tile, for the tile's climate and for the height bounds of a cap inside the tile is its source's (equal values; the samples bit for bit). A stamp stage (GEN-026) SHALL pass a tile through exactly when the footprint it declares for the tile (WDEF-007, with the stage's halo) is empty, , never for a root tile (the normal stencil of a coarse mesh reaches past the declared footprint there) and never for tile ids of another face mapping than the cell grid's, and a wrapper of a field SHALL forward the answer with the chain (a field that overrides the read set or the body does not forward it). A stage that changes its output where no item reaches SHALL NOT claim to pass any tile through.

Verify: A
Status: active
Source: ADR 0068, ADR 0066 points 1 and 4

#### Scenario: Identity
- **WHEN** a stamp stage has no item in reach of a tile, and a coarse tile away from the stamp, and another over it
- **THEN** the first two are passed through and their samples, bounds and climate equal the base's bit for bit, and the third is not passed through

### Requirement: GEN-032 The stage's field-wide bound is read once per item revision
A stamp stage's `max_abs_height_m` (the base's bound plus the sum of the layer's `max_delta_m`) SHALL be computed from the items once for each item revision (`ItemReader::revision`) and read from the stage afterwards. A plan asks for it per tile and per level; SPIKE-29 found the walk over every item was 99.8 % of a plan job at 1,000 items. The value SHALL equal the base's bound plus `ItemReader::max_delta_sum` at the call after items are added, removed, changed or moved between layers. The cache is per stage instance and safe for the plan job's threads. No output changes.

Verify: A
Status: active
Source: opus-planner (2026-10-22, SPIKE-29 fix 1), GEN-026, STRM-027

#### Scenario: The cache equals the walk
- **WHEN** items of the layer are added, one of another layer is added, one's `max_delta_m` is changed, items are moved into and out of the layer and one is removed, the stage's bound being asked after each
- **THEN** every answer, the first and a repeat from the cache, equals the base's bound plus the uncached sum over the items at that moment

### Requirement: GEN-033 The height fields follow the relief scale
The terrain and the eroded field SHALL read the bake's elevation multiplied by the body's relief scale (WDEF-017), rounded to `f32` once per lattice sample and passed through unchanged at 1 (`ScaledLattice`); continentalness, climate and the ocean channel are the bake's own. Their largest height, their height bounds over any cap, their slope bounds and their `inputs_hash` SHALL follow the scale: every height lies inside the bounds of every cap that holds its direction and inside `max_abs_height_m`, and each scale (and none) has its own key. A bake is not re-made for a changed scale.

Verify: A
Status: active
Source: owner (2026-10-22, via opus-planner), WDEF-017, CON-11, GEN-016, GEN-017

#### Scenario: The base elevation
- **WHEN** the terrain field at scale 2.5 and at 1 are asked the base elevation at 300 random directions
- **THEN** the first is 2.5 times the second within 5 mm and keeps its sign

#### Scenario: Bounds and keys
- **WHEN** both fields at no scale, 1.5 and 4 are asked the bounds of random caps and the heights inside them
- **THEN** every height lies in its cap's bounds and under the field's largest, the largest at 4 exceeds 36 km, the three keys differ and a world without the node keeps the default key

### Requirement: GEN-040 The global coupled run
`generators::coupled` SHALL run stream-power landscape evolution on the cube-sphere bake lattice (ADR 0077 point 1): per step a priority flood fills the depressions, each land cell takes the steepest receiver down the filled surface with a jitter `1 + j (2 h - 1)` (`h` a hash of seed, cell and direction), the drainage area is accumulated downstream, and the heights take the implicit update `z = (z + dt U + f z_r) / (1 + f)`. The run SHALL be single-threaded in a fixed order with `libm`, so its results are bit-identical across runs and platforms (CON-14); every receiver is lower on the filled surface; the outlets drain the land area.

Verify: A
Status: active
Source: opus-planner (2026-10-23, M8-1), ADR 0077, SPIKE-33, SPIKE-35

#### Scenario: Network, area and determinism
- **WHEN** a 12-cell lattice with a 60 % land world runs 30 steps twice, with jitter 0 and 0.8
- **THEN** no receiver drains uphill and no chain cycles, the outlets' area equals the land area, both runs agree bit for bit, the mean change per step falls, and the jitter changes the receivers

### Requirement: GEN-041 The coupled run's debug maps
`generators::coupled` SHALL draw the heights and the drainage area of a face of the coupled run as images: heights as the grey of the height debug view (sea at or below 0 in blue), the area on a log scale from the area of one cell (dark) to the largest area on the face (bright), so the river network reads as the bright lines. The maps are a pure function of the arrays.

Verify: A
Status: active
Source: opus-planner (2026-10-23, M8-1 part c), ADR 0077

#### Scenario: What the maps show
- **WHEN** the maps of a run on a small lattice are drawn
- **THEN** a sea cell is blue, a land cell is grey by height, the cell with the largest area is the brightest and a cell with one cell's area the darkest, and two draws are equal

### Requirement: GEN-034 Chunk scope and key
`Scope` SHALL have a `Chunk(ChunkId)` variant, distinct from `Global` and `Tile` in the key (its own scope word), where a `ChunkId` is a face and the (i, j) of a tile of the chunk grid's level. A chunk field's stage key (STRM-011) SHALL cover the grid level, the halo in metres and the step cap, so changing any of them re-keys every chunk of the field; a regional stage with `Chunk` scope SHALL fit, and one with `Global` scope SHALL not.

Verify: A
Status: active
Source: ADR 0077 points 2 and 4, ADR 0022, STRM-011, CON-08, CON-14

#### Scenario: Re-keying
- **WHEN** a chunk field's grid level, halo (bits) or step cap changes, or a tile and a chunk have the same raw numeric id
- **THEN** exactly the keys of that field's chunks change, and the tile and the chunk never share a key

### Requirement: GEN-035 Chunk grid and apply level
Chunks SHALL lie on a fixed grid of the tiles of level 11 (whole coarse cells, about 5 km), and a chunk field SHALL state its raster cell size; its apply level SHALL be the coarsest tile level whose vertex spacing is at most twice that cell size (level 15 for 8 m cells, level 16 for 4 m), computed from the raster and never set by hand. The raster is read unfiltered from the apply level down (GEN-053).

Verify: A
Status: active
Source: ADR 0077 point 5, SPIKE-34 (conditions 1 and 3), STRM-029

#### Scenario: Apply level
- **WHEN** a chunk field has 8 m cells, then 4 m cells
- **THEN** its apply level is 15, then 16

### Requirement: GEN-036 Level-aware height
The height capabilities of a field SHALL include `height_at_level(dir, level)`; for a field without chunk detail it SHALL equal `height(dir)` at every level, and for a chunk field it SHALL be the base plus the chunk change read from the pyramid at `level` for a level at or finer than the pyramid's top level (GEN-035), and the base for a coarser level. `height` SHALL remain position-only and SHALL equal `height_at_level` at the field's finest level. Tile generation SHALL read `height_at_level` with its own level.

Verify: A
Status: active
Source: SPIKE-34 section 3, ADR 0077 point 5, GEN-020

#### Scenario: Default
- **WHEN** every existing field is sampled with `height_at_level` at levels 0 to 22
- **THEN** each value equals `height(dir)` bit for bit

#### Scenario: Chunk field
- **WHEN** a chunk field with a ready chunk is sampled over the chunk at levels 13, 14 and 15, and at a level that reads the raster (finer than the apply level), with the top level at its default 14
- **THEN** level 13 equals the base bit for bit, levels 14 and 15 read the box-filtered pyramid (base plus change), and the finest level equals the base plus the chunk's stored change at the cell centres

### Requirement: GEN-037 Chunk pyramid steps
A chunk's change SHALL be stored with a pyramid down to the apply level, each level box-filtered from the level below it (never from a coarser run). A tile at the apply level SHALL differ from its parent, at the parent's vertices, by at most 0.25 m at p99 for the stand-in compute; the steps between the coarser levels up to the pyramid's top are measured and recorded (design.md), not bounded.

Verify: A
Status: active
Source: SPIKE-34 section 3, M8 plan criterion 3

#### Scenario: Step against the parent
- **WHEN** the stand-in chunk (30 m amplitude, 8 m cells) is ready and every tile of levels 14 and 15 over it is generated
- **THEN** the height difference between a level-15 tile and its level-14 parent at the parent's vertices has p99 at most 0.25 m, and the test prints the p99, the maximum and the tile ids of the worst vertex on failure

### Requirement: GEN-038 Chunk border agreement and determinism
Neighbouring chunks SHALL agree on every shared border sample, across cube-face edges and at corners, and a chunk's result SHALL be bit-identical for 1, 2 and 8 threads and on Windows and Linux (fixed order, `libm`). A chunk field SHALL state its halo in metres as at least its step cap times its reach per step.

Verify: A
Status: active
Source: ADR 0077 points 3 and 4, GEN-004, GEN-002, GEN-003, CON-14, M8 plan criterion 2

#### Scenario: Borders
- **WHEN** two neighbouring stand-in chunks are computed with the stated halo, in the middle of a face, across a face edge and at a cube corner
- **THEN** their border rows agree to the bit, and with a halo below the rule the test reports the largest difference in metres and the chunk ids

#### Scenario: Threads and systems
- **WHEN** a chunk is computed on 1, 2 and 8 threads
- **THEN** its hash is identical and equals the recorded known answer, which CI checks on Windows and Linux

### Requirement: GEN-039 The chunk stage's position per layer
A layer SHALL have a `chunk_stage` setting, `OnTop` by default: the chain is base, chunk, stamps, so an edit never invalidates a chunk. `Under` (base, stamps, chunk) SHALL be representable in the key but refused with an error naming the layer until the editor supports it (M8-7, after the owner's confirmation of D3).

Verify: A
Status: active
Source: M8 plan D3 (provisional), ADR 0077 point 7

#### Scenario: Default and refusal
- **WHEN** a world has a layer without the setting and one set to `Under`
- **THEN** the first builds with `OnTop`, and the second is refused with an error naming the layer

### Requirement: GEN-053 The chunk pyramid's top level
The chunk pyramid SHALL reach from the apply level up to a top level, by default tile level 14 (a parameter of the chunk field: level 11 needs about 285 chunks per view, level 14 about 5). A tile coarser than the top level SHALL be identity on the base field and key as the base (STRM-029). A tile at or finer than it SHALL read the pyramid at the continuous level of its vertex spacing over the cell size (GEN-037), the raster itself where that is 0.

Verify: A
Status: active
Source: planner decision (option a, 2026-10-24), ADR 0077 point 5, SPIKE-34 section 3

#### Scenario: Top level
- **WHEN** a chunk field with its top level at 14 and a ready chunk is sampled at levels 13, 14 and 15, then with the top at 11
- **THEN** level 13 equals the base bit for bit and levels 14 and 15 read the pyramid; with the top at 11, levels 11 to 13 read it too

### Requirement: GEN-054 Heights of a field with chunk detail
For a field with chunk detail the single-height rules (GEN-016, GEN-020) SHALL hold at each tile's own level: `height_at_level` of a tile's level is the value its tile, its batch (`heights_and_climate`, `heights_threaded_at_level`) and its origin carry, bit for bit, and the same point differs between a tile and its parent by the pyramid step of GEN-037. `height` SHALL be the value at the finest level. A field without chunk detail SHALL keep its one batch and its single height at every level.

Verify: A
Status: active
Source: GEN-036, GEN-016, GEN-020, LOD-009

#### Scenario: Tile, batch and origin
- **WHEN** a chunk field's tile, its batch heights and its plan origin are compared with `height_at_level` at the tile's level
- **THEN** they are equal bit for bit, and a field without chunk detail gives the same values at every level

### Requirement: GEN-055 A field with a chunk stage names it
The field built for a world with a chunk stage in its chain (GEN-039) SHALL report the implementation id `chunk-stage` (as a stamp stage reports its own, CON-07); tiles at or finer than the pyramid's top level SHALL be keyed by the chain (STRM-038), and a world without a chunk stage SHALL build the field GEN-018 describes.

Verify: A
Status: active
Source: GEN-018, CON-07, STRM-038

#### Scenario: Id
- **WHEN** a world's field is built without and with a chunk stage
- **THEN** the first reports the resolved id and the second `chunk-stage`

### Requirement: GEN-044 The chunk horizon map
`generators` SHALL bake, for a raster of heights with its halo, the horizon map of the chunk: per texel and per azimuth `k` of `A` evenly spaced azimuths from north, the elevation angle of the highest terrain along that azimuth out to the range, found by marching with steps that grow 4.5 % plus 2 m. The bake SHALL be a pure function of the raster, the halo and the parameters, independent of the thread count (CON-14).

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), SPIKE-32 step 2, ADR 0077

Detail: The defaults are 4 m texels, `A` = 16, a range of 5 km and steps of 2 m plus 4.5 % of the distance; each is a parameter. Terrain at distance `d` SHALL count `d^2 / (2 R)` lower for a body of radius `R`. The angle is stored as a byte, 90 degrees over 255 steps, 0 at or below the horizon. Azimuths are in the raster's frame (`i` east, `j` north, azimuth 0 along `j`); the raster of a chunk on another cube face is rotated against its neighbour's, so GEN-046 maps the sun's azimuth into the chunk's frame and a texel shared across a face edge is compared after that mapping. A block with less than `ceil(range / texel) + 2` texels of raster on any side, a raster whose size does not match, and parameters that are not finite and positive SHALL be refused with the cause named. The bake SHALL be a pure function of the raster, the halo and the parameters, single-threaded per texel row set in a fixed order or independent of the thread count (CON-14), with `libm`, and texels shared by neighbouring chunks SHALL be identical. Stated limits: the march steps `texel_m` metres per texel along `i` and along `j`, but under the tangent warp a texel of a face-coordinate raster is up to 1.41 times as long along `i` as along `j` near the middle of a face edge (1.0 at the face centre), so the marched distances, and with them the horizon angle, are off by up to that factor along one axis there (a bias of a few degrees at the worst chunks; the direction of the lookup is exact, GEN-046); fixing it means marching in metres through the true tangent vectors. A map that the renderer reads (REND-038) SHALL have exactly 16 azimuths: the packers refuse any other count.

#### Scenario: Known horizons
- **WHEN** the map is baked on a flat raster with one ridge of known height and distance
- **THEN** texels looking toward the ridge have the angle `atan(height / distance)` within the step and texel error, texels looking away from it have horizon 0, the result is equal for every thread count, a block baked from a raster that holds its halo equals the same block of a larger raster, and a raster without the halo changes the edge texels

### Requirement: GEN-045 The coarse horizon from the global run
`generators::coarsehorizon` SHALL compute, from the stored heights of the coupled run (STRM-043), the coarse horizon of the cells of an observer lattice of `m` cells per face edge: per cell and per azimuth the elevation angle of the highest sample of the run along the great circle in that azimuth, out to the far range (default 50 km, 16 azimuths). The result SHALL be a pure function of the stored heights and the parameters, independent of the thread count, and stored as a global channel (STRM-045).

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), SPIKE-32 architecture finding 2

Detail: `generators::coarsehorizon` SHALL compute, from the stored heights of the coupled run (STRM-043), the coarse horizon of the cells of an observer lattice of `m` cells per face edge (a divisor of the run's): per cell and per azimuth the elevation angle (the chord above the observer's horizontal plane, on the sphere) of the highest sample of the run along the great circle in that azimuth, out to the far range (default 50 km, 16 azimuths, samples one cell of the run apart, at most 256 azimuths, a range of at most a quarter of the way round the body), where each sample is the run's height at the sample's direction (so a march crosses a cube face edge without face coordinates) and sea at or below 0 counts as 0; the observer's own height is the mean over the run cells its cell covers. The angle is the byte of GEN-044. Azimuths are in the geographic frame (0 toward the pole axis, a quarter turn east), unlike GEN-044's raster frame; GEN-046 rotates the sun's azimuth into each. The result is independent of the thread count, and a wrong size, a bad `m` or parameters that are not finite and positive are refused with the cause named. The result SHALL be a pure function of the stored heights and the parameters and be stored as a global channel beside the others (STRM-045). Stated limit: each cell is baked in the geographic frame of its own centre while the lookup (GEN-046) uses the fragment's own geographic frame, so within about 5 coarse cells of a pole (about 100 km at 512 cells) the frames differ by up to the longitude difference and the horizon read at an azimuth can come from another bearing; the fix is a pole-free frame such as the face raster frame.

#### Scenario: A far peak
- **WHEN** the coarse horizon is computed on a small body with one wide plateau on a cube edge, and on a lattice of varied heights with sea
- **THEN** cells within range see the plateau toward it at its elevation angle (between the angles of its far and near edges), cells on the other face see it across the edge, cells beyond the range see nothing, a flat body has no horizon, the result is the same for every thread count, and deeper sea changes nothing

### Requirement: GEN-046 The horizon of a point
`generators::horizon_lookup` SHALL be the CPU reference of the shader's shadow (CON-07). The horizon at an azimuth is read by linear interpolation between the two nearest stored azimuths (wrapping from the last to the first); the horizon of a point is the larger of the chunk map's and the coarse map's, a map that is not there (the chunk's has not landed) leaving the other alone and neither giving 0; the lit factor is 0 when the sun's elevation is 0.35 degrees or more below the horizon, 1 when 0.9 degrees or more above it, and a smoothstep between.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), CON-07

Detail: The sun's azimuth SHALL be formed in each map's own frame: the geographic frame for the coarse map, the cube face's raster frame (`i` east, `j` north, which the warp shears by up to 30 degrees and of which `i` is up to 1.41 times as long as `j` near an edge) for the chunk's, where the sun's horizontal part is solved for its components along the two axes at their true lengths and the azimuth is taken in index space, the space the bake marches in. The shader SHALL equal this within the bounds stated in REND-038 (its numbers, at texel centres).

#### Scenario: Combination
- **WHEN** a point has a chunk horizon of 3 degrees and a coarse horizon of 5 degrees at an azimuth, and the sun is 4 degrees and then 6 degrees high there
- **THEN** the point is in shadow at 4 degrees and lit at 6 degrees, and with the coarse horizon absent the chunk map decides alone (and the coarse map alone before the chunk's lands), the ramp is monotone between its ends, the interpolation is exact at the stored azimuths and wraps, and the sun's azimuth is 0 toward north, a quarter turn toward east in both frames and differs between the two frames on a turned face

### Requirement: GEN-047 The global network in the chunk raster
A chunk stage of the A-net kind SHALL take the river network of a chunk from the stored global coupled run: the path from every coarse cell above `river_area_m2` (GEN-068) to its receiver, node to node (GEN-066), is the chunk's corridor, so neighbouring chunks find the same path through their shared border. The drainage area crossing the halo ring is injected as inflow, the initial fine heights are the coarse heights upsampled plus the base field's detail, and the corridor's bed is set by GEN-059. A chunk with no such cell SHALL have no corridor.

Verify: A
Status: active
Source: ADR 0077 points 1 and 3, SPIKE-33 (two-scale scheme), SPIKE-35 item 1, GEN-040, STRM-043

#### Scenario: One path for two chunks
- **WHEN** two neighbouring chunks are given the same stored run
- **THEN** their corridor polylines agree at the shared border to the bit, each starts and ends at a coarse cell centre (with no slide and no routing, else at the node of GEN-066), and the inflow at each ring equals the coarse area that crosses it

### Requirement: GEN-048 The breach rule leaves no uphill channel
After the incision no channel cell (drainage area over 50,000 m2) SHALL have a receiver higher than itself by more than 1 micrometre, except a link between two fixed corridor cells, which may climb by at most 5 cm (GEN-059). The rule is: channel cells take no diffusion and no slumping, a cell whose receiver is a higher ring cell is an outlet, and along the corridor's links a cell drains to the next cell of the path.

Verify: A
Status: active
Source: SPIKE-35 item 1 (variant c4), ADR 0077 consequences

#### Scenario: Zero uphill
- **WHEN** a chunk of a terrain whose corridor runs uphill in places runs N = 30 steps
- **THEN** 0 channel cells have a higher receiver, and two runs give the same receivers and heights, bit for bit

### Requirement: GEN-049 The halo of an A-net chunk
An A-net chunk SHALL state its halo (3 km by default) and SHALL, with it, equal the same run over a wider halo in every cell of its core: bit for bit where every channel chain meets a fixed cell (an anchor) inside the halo and every catchment lies within the area window, and to within 2 cm on a gentle valley where a stream runs a long way to the river. The reaches that bound this (breach window, area window, tributary step cap) are parameters of the stage and part of its definition.

Verify: A, B
Status: active
Source: SPIKE-35 item 2, ADR 0077 point 4, GEN-038

#### Scenario: Halo
- **WHEN** two neighbouring chunks run with halos of 1.5, 3 and 5 km against the whole-region run
- **THEN** the cores differ in 0 cells at 3 and 5 km, and the test prints the number of differing cells and the largest difference in metres at 1.5 km

### Requirement: GEN-050 Jittered receivers
The receiver of a cell SHALL be the steepest of its eight neighbours after scaling the slope of direction `k` by `1 + j (2 h - 1)`, `h` in `[0, 1)` a hash of the cell and `k`, with `j` 0.4: a function of position only, the same as the global run's jitter, so every chunk makes the same choice. The share of axis directions among the receivers of cells over 2e4 m2 SHALL be at most 0.56.

Verify: A
Status: active
Source: SPIKE-35 item 3, GEN-040

#### Scenario: Axis share
- **WHEN** the receivers of a 4 m lattice of 49 km2 are chosen with j 0.4 and with j 0
- **THEN** the axis share with jitter is at most 0.56 and below the share without, and two runs give the same receivers

### Requirement: GEN-051 Diffusion and slumping
After the incision a chunk SHALL apply hillslope diffusion and thermal slumping to its non-channel cells in a fixed order with `libm` only: diffusion within its stability bound, slumping moving the excess over the angle of repose to the lower neighbours for a fixed number of passes. Channel cells SHALL be unchanged by both, and the result SHALL be bit-identical for 1, 2 and 8 threads.

Verify: A
Status: active
Source: SPIKE-33 stage model, ADR 0077 point 3, CON-14

#### Scenario: Hillslopes
- **WHEN** a chunk with steep slopes and a channel runs the processes
- **THEN** the channel cells are unchanged, no non-channel cell exceeds the angle of repose by more than one pass can move, the volume moved equals the volume received, and the hash is the same on 1, 2 and 8 threads

### Requirement: GEN-052 A-net chunk time
An A-net chunk of 24 km2 of core SHALL take at most 4 s at 8 m cells with the 3 km halo (1.8 million cells) on the reference machine, one thread, measured by the performance harness through the architect and never in a test (CON-20); the time at 4 and 10 m is recorded with the run (4 m was about 14 s per chunk on a synthetic scene: not met, ADR 0077 allows 4 to 10 m).

Verify: C
Status: planned
Source: SPIKE-33, M8 plan risks, CON-22, planner decision 2026-10-24 (8 m default)

#### Scenario: Chunk time
- **WHEN** a chunk of 24 km2 at 8 m cells runs on the reference machine in a perf window
- **THEN** it finishes within 4 s at 8 m cells and the log holds the times at 4 and 10 m

### Requirement: GEN-057 Corridor anchors follow the drawn ground
The corridor of an A-net chunk SHALL be anchored at every polyline point at the base field's height there minus the valley depth `depth_m (A / area_ref_m2)^exponent` for the river's area `A` at that point, at most `lerp(cap_low_m, cap_m, rf)` with `rf` the relief factor (GEN-060; defaults: 3 m at 100 km2, exponent 0.5, `cap_m` 100 m, `cap_low_m` 30 m, so the cap is at most a third of `bound_m`, 300 m), and GEN-059 sets the bed from these raw anchors. The coupled run's absolute height SHALL not set an anchor. The law's constants are part of the stage's definition hash.

Verify: A
Status: active
Source: owner decision 2026-10-25 (terrain relief into M8, part 1), GEN-047, learnings 2026-10-25

#### Scenario: High ground over a low run
- **WHEN** a chunk of a base 2,600 m above the coupled run runs its incision
- **THEN** every anchor lies at most `cap_m` under the base, no core cell is cut by half the bound or more, and the depth of the law rises with the drainage area up to the cap

### Requirement: GEN-058 The banks settle to the angle of repose
After the incision steps an A-net chunk SHALL lower every free cell within `bank_rounds` (`ceil(bound_m / (talus cell_m)) + 6`, 48 at 8 m) rings of the core to at most `z(s) + talus x distance` of every protected cell `s` that is not in the outer ring (a channel, the corridor, an anchor), but not under a protected neighbour by more than `talus x distance`, and SHALL not move any protected cell. A cell SHALL only be lowered, and the core SHALL be the same for every halo of at least the stated one.

Verify: A
Status: active
Source: planner decision 2026-10-25, GEN-051, learnings 2026-10-25 (walls), ADR 0074

#### Scenario: A breach cut through a ridge
- **WHEN** a chunk of a river that crosses a 600 m ridge runs with and without the settle
- **THEN** without it more than ten bank pairs stand over the repose slope, with it at most one in twenty, no protected cell differs, and no cell is higher with it than without

### Requirement: GEN-059 The bed of a river follows the ground and falls the whole way
Every corridor cell of an A-net chunk SHALL be held at a bed interpolated along its segment from samples at every polyline point: the base height minus the depth law (GEN-057), as the running minimum from upstream along the main stem (the donor of the largest area) over `window` segments, each sample a least fall under the one above and at most `cut_cap_m` under the ground. `window` (16), `cut_cap_m` (200 m) and the fall law are part of the definition hash.

Verify: A
Status: active
Source: SPIKE-40, SPIKE-42, ADR 0081, ADR 0082 (proposed), GEN-057

#### Scenario: A ridge across the river
- **WHEN** a chunk of a river over a base with a ridge across its path runs
- **THEN** every corridor bed is at most the ground, no link climbs by more than 5 cm, and the cut is at most the cut cap plus the valley depth

### Requirement: GEN-069 A tributary hangs above the stem and no bed climbs
A tributary's bed SHALL end no lower than the main stem's bed at the confluence, rising at the least fall back up the river, and never over the ground. No corridor bed SHALL lie over the ground, no link SHALL climb by more than 5 cm and no linked corridor cell SHALL lack a receiver, wherever the ground does not rise along the river by more than the cut cap.

Verify: A
Status: active
Source: SPIKE-42, GEN-059, GEN-068

#### Scenario: A fork over a hollow
- **WHEN** a chunk of a fork with a tributary runs over smooth ground and over a 30 m hollow on the tributary
- **THEN** no bed lies over the ground, and over smooth ground no link climbs by more than 5 cm

### Requirement: GEN-060 The channel from discharge, the valley from relief
Round every corridor cell an A-net chunk SHALL hold the cells within `band_cap_m` of the river at a channel and valley profile: a channel of width `w_coef Q^w_exp` and depth `d_coef Q^d_exp` (`Q = q_per_km2 A`, km2) under a valley floor `lerp(floor_low, floor_high, rf)` channel widths wide at the bed, the corridor cells at the channel floor, and sides rising at `lerp(side_low, side_high, rf)` to the ground. `rf` is the base's relief over the river's coarse cell and its eight neighbours over `relief_full_m`; the valley depth follows GEN-057; the cap follows the halo (`(halo_rings - area_window - 2)` cells at most; 0 paints no band). The constants are part of the definition hash.

Verify: A
Status: active
Source: SPIKE-40, SPIKE-41, planner decision 2026-10-25 (H2), GEN-057, GEN-058

#### Scenario: A great lowland river
- **WHEN** a chunk of a river of 9.5e6 km2 over smooth ground runs its incision
- **THEN** the channel is within a factor 2 of the law's width at half depth and its depth within 1.5 of the law's, in a valley floor wider than half the channel

### Requirement: GEN-061 The fine creases follow the landform
The erosion field (`cpu-erosion-v1`) SHALL split its crease octaves at wavelength 1.6 km (octave 4 of 25 km spacing halving): the coarse octaves SHALL keep the amplitude `A(E) carve(slope)`, the fine octaves the amplitude `A(E) (carve + (1 - carve) L)` with `L` the landform driver in `[0, 1]` read from the landform lattice (GEN-062, the interpolant of `global::landform(seed, direction)`). With `L` 0 the detail SHALL be the detail of version 1; with `L` 1 only the fine creases differ, and on ground where `carve` is 1 nothing differs. The field's version SHALL be 2.

Verify: A
Status: active
Source: ADR 0080 point 2, owner decision 2026-10-25 (terrain relief into M8), GEN-064

#### Scenario: Plain and range zone
- **WHEN** the detail is evaluated at the same ground elevation and bake slope with `L` 0 and `L` 1
- **THEN** the coarse part is equal and the fine part changes by `A (1 - carve) L` times the fine crease sum

### Requirement: GEN-062 The landform is read from a fixed lattice and bounded over a cap
The erosion field SHALL read the landform from `global::LandformLattice`: the samples of `global::landform` (rounded to f32) at the face coordinates `2 i / 256 - 1` on each cube face, interpolated bilinearly, a pure function of `(seed, direction)` with the same bits in every tile and continuous across face edges. Its range over a cap SHALL be the least and greatest sample of the cells the cap touches, and `[0, 1]` for a cap over 0.05 rad, over 16 cells an axis or reaching a face edge; the field's height bounds SHALL take the landform in that range.

Verify: A
Status: active
Source: GEN-017, GEN-061, ADR 0074 (bounds must be sound)

#### Scenario: Random caps
- **WHEN** 20,000 random caps of radius 1e-6 to 0.01 rad, rims included, and cube edges and corners crossed are taken
- **THEN** the lattice value at the centre and at a point of the rim lie in the range of the cap, the lattice differs from `landform` by less than 0.03, and two points 1e-12 either side of a cube edge or corner differ by less than 1e-9

### Requirement: GEN-066 The main stem is routed over the ground
The A-net chunk stage SHALL route a river segment over the base. The node of a river's coarse cell SHALL slide to the lowest raster ground (every `route_stride`-th cell) within `lerp(slide_m, slide_steep_m, rf)` of the cell's centre, a head to the highest ground within `head_m`. The segment from node to node SHALL be the least-cost path (rise in millimetres plus `route_lambda` times length, integer arithmetic) over the raster cells within `route_band_m` of the coarse guide. Every segment through a coarse cell SHALL share its node; `route_band_m` 0 with no slide gives the coarse path. The parameters (slide 0.5 to 1 km, band and head 1 km, lambda 0.02) are part of the definition hash.

Verify: A
Status: active
Source: ADR 0081, ADR 0082 (proposed), SPIKE-42

#### Scenario: A ridge across the coarse path
- **WHEN** a river's coarse path crosses a ridge that a path within the band avoids
- **THEN** the corridor of the routed chunk reaches a lower highest ground than the coarse path's by at least 30 m

### Requirement: GEN-067 Chunks agree on the routed corridor and thread count changes nothing
Two chunks that see the same coarse segment SHALL find the same routed polyline, the same corridor cells, links and beds in the cells they share (bit equal), whatever the size of their halo, and the routing SHALL use no threads of its own and no state a result depends on: its memo (nodes, relief factors and routed polylines, shared by the chunks of one stage and emptied when the law changes) holds pure functions of the coarse cell and segment, a hit equal to a miss bit for bit, so the result is the same at any thread count.

Verify: A
Status: active
Source: ADR 0081 consequences, GEN-047, CON-14

#### Scenario: Neighbouring chunks
- **WHEN** two neighbouring chunks of a river with slid nodes are built
- **THEN** every shared cell has the same corridor flag, bed bits and next cell

### Requirement: GEN-068 The coarse run's tributaries are routed like stems
The A-net chunk stage SHALL route every coarse link whose drainage area exceeds `river_area_m2` (default 100 km2; the stage's definition sets 5e7 m2) by the node slide, least-cost segment, bed rule and valley law of the main stems (GEN-059, GEN-066), so topology, crossings and areas come from the global run and chunks agree on them by construction (GEN-067). `river_area_m2` is part of the definition hash and `ANET_VERSION` is 7.

Verify: A
Status: active
Source: ADR 0081 point 3, planner decision 2026-10-25

#### Scenario: A tributary under the threshold
- **WHEN** a chunk of a river with a tributary of 3e8 m2 is built with thresholds of 1e8 and 5e8 m2
- **THEN** the tributary has a corridor with the first and none with the second

### Requirement: GEN-070 Slope, curvature, wetness and moisture of a chunk
`generators::drivers` SHALL compute, for every core cell of a chunk raster from the cell and its eight neighbours at their true distances (the lattice is equal-angle, and across a face edge the neighbour found is the real cell), the slope, the mean curvature and the wetness `ln(A / (w tan s))` with `tan s` floored at 0.01 and the drainage area `A` clamped to 1e8 m2, and the moisture `clamp((W - 4) / 12, 0, 1)`. A non-finite height or area SHALL be refused. The functions SHALL be pure, use `libm` only and no threads.

Verify: A
Status: active
Source: ADR 0076 points 2 and 3, SPIKE-30, planner decision 2026-10-26

#### Scenario: Known rasters
- **WHEN** the drivers are computed on a plane, a bowl, a saddle, a cone and a hollow whose area exceeds the clamp
- **THEN** the slope and curvature equal the geometry's, a flat gives the wetness of the floor, the hollow the maximum, a plane on cells 0.7 as wide gives the same drivers, a ramp on a cube-face edge gives the same codes at the edge as two cells in, and a second run on another thread gives the same bits

### Requirement: GEN-075 Soil depth of a chunk
The soil depth of a core cell SHALL be `soil_base_m` (2.5 m) times a slope factor `exp(-slope / 18 degrees)`, a curvature factor `1 + 0.5 tanh(100 m kappa)`, a flow factor from 0.5 to 1 over the drainage area 1e2 to 1e8 m2, a lithology factor `1 + 0.5 n` and an altitude factor falling to 0 from 2,500 to 4,500 m, clamped to 0 to 8 m and stored square-root coded in 8 bits. The lithology `n` SHALL be `height::layer_noise` at the cell's f64 unit direction times 400, seeded with the world's seed (`drivers.seed`, set by the stage's owner). The constants SHALL be part of the stage's definition hash.

Verify: A
Status: active
Source: ADR 0076, SPIKE-30, planner decision 2026-10-26, CON-11

#### Scenario: Factors
- **WHEN** the soil depth is evaluated on a paraboloid of curvature 0.01 per metre, on steep ground, with more flow, high up and for the two ends of the lithology
- **THEN** the curvature factor is 1.3808, steep and high ground are thin, more flow and a hollow are deeper, and the codes round trip within half a code step

### Requirement: GEN-076 Annual-mean insolation of a chunk
The insolation of a core cell SHALL be the annual-mean beam irradiance on a tilted plane (Duffie and Beckman's cosine, every fifth day of the year in 96 steps of the day, the sun above the horizon only, no shadowing by terrain, never the current sun) in units of the flat plane at the equator, read bilinearly from a 17 x 17 table over slope (0 to 60 degrees, steeper cells read 60) and aspect at the latitude of the chunk's centre.

Verify: A
Status: active
Source: ADR 0076, SPIKE-30, planner decision 2026-10-26

#### Scenario: Against a brute force
- **WHEN** the insolation is compared with a one-minute integration over every day of the year at the equator and at 45 degrees north up to 60 degrees of slope
- **THEN** it is within 1 %, and the table reads the function it tabulates and wraps the aspect

### Requirement: GEN-071 Driver channels of a chunk and their blend
A chunk result SHALL carry the soil depth, the moisture and the insolation as three 8-bit channels at the raster with box-filtered pyramid levels, in the chunk's disk record. A consumer SHALL read a driver as `neutral + w (chunk - neutral)` with `w` the chunk weight that fades the height; the neutral values are soil 1 m, moisture 0.5 and the insolation of the latitude alone.

Verify: A
Status: active
Source: ADR 0076, GEN-037, planner decision 2026-10-26

#### Scenario: The range edge
- **WHEN** the weight of a chunk goes from 0 to 1
- **THEN** the driver goes from the neutral value to the chunk's value, monotone, with no step

### Requirement: GEN-072 The materials read the drivers
`MaterialParams::weights` SHALL read the soil depth, the moisture and the insolation from the blended drivers in its reserved inputs, by the same function and the same input vector as for the other inputs.

Verify: A
Status: active
Source: LOOK-008, ADR 0076 point 5

#### Scenario: A hollow
- **WHEN** a look gives soil a curve on the moisture input and the drivers describe a wet hollow
- **THEN** soil takes weight in the hollow and none on the dry flank

### Requirement: GEN-073 The default look ignores the drivers
The default look SHALL give no curve to the soil-depth, moisture and insolation inputs, so its material weights and albedo SHALL be bit-identical for every driver value, and the images of every committed golden SHALL be unchanged with no candidate; the neutral values matter only once a look gives those inputs curves.

Verify: A, C
Status: active
Source: planner decision 2026-10-26, ADR 0017

#### Scenario: Today's goldens
- **WHEN** the default look's weights are evaluated for several driver vectors and the committed goldens are rendered
- **THEN** the weights are equal bit for bit and every image equals its golden

### Requirement: GEN-074 The tile driver block and the driver views
A compact tile SHALL carry a driver block of 9 x 9 words (GEN-025): at each climate texel the soil depth, moisture and insolation codes of the ready chunk under it as the tile's level reads them, or the neutral codes where none is ready, and the neutral insolation of the texel's latitude in the top byte. The fragment stage SHALL read the block bilinearly, blend it as `neutral + w (chunk - neutral)` with `w` one minus the node's fade, and fill the reserved material inputs (GEN-072). The debug views `soil`, `moisture` and `insolation` (`ShadeView` 7 to 9, reached by J) SHALL show the blended drivers in false colour.

Verify: A, C
Status: active
Source: ADR 0076 point 5, GEN-071, GEN-072, GEN-025

#### Scenario: Under a chunk and away from one
- **WHEN** compact tiles are built over a ready chunk with drivers and away from every chunk
- **THEN** the first hold the chunk's codes in every texel, the others the neutral codes, a tile with chunk drivers has another content hash, and a tile under a chunk that moved no height gets a plane of zeros so its drivers fade in

#### Scenario: The views
- **WHEN** a compact tile whose driver block is uniform, and one whose block is a ramp, are drawn in each driver view
- **THEN** every covered pixel has the colour the CPU mirror gives for those codes (within 0.003 for the uniform block), and a node with no fade plane reads the chunk's values at full weight

#### Scenario: Constants
- **WHEN** the block is decoded
- **THEN** the soil code uses the default `soil_max_m` of 8 m in the builder and the shader alike
