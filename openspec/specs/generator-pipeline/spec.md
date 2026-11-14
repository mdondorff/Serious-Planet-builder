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
Generation SHALL be staged global-coarse, regional (per tile with a halo), local (per point), and a stage SHALL read only outputs of earlier stages: a field graph SHALL be refused when a field declares a read of a field of its own stage or a later one, of an unknown field, or the same input twice, when a field is declared twice, or when a halo is not a finite fraction in [0, 1] (regional fields only; `-0.0` is `0.0`), with an error naming the fields and stages. The key of a field's output SHALL cover the stage key version, the field's stage, id, version, implementation id and halo, its scope (the whole body for the global bake, a tile for regional and local outputs, the kind part of the key, and a scope of the wrong kind for the stage a bug that panics), the body, reference surface, face mapping and resolution, the recorded read set and the keys of the outputs it reads, so that changing one field's version, implementation id or halo re-keys exactly the fields downstream of it, and a key built without one of its inputs' keys SHALL be a bug that panics rather than a stale tile. The key layout SHALL carry its own version and pinned known answers. The graph checks declared reads; the reads contract test is the GEN-026 scenario "Reads contract over the authored stack" (ADR 0021 point 7).

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

### Requirement: GEN-014 Field bake time and memory
A field bake (formerly "global bake") at 1 km cells or finer (16384 per face edge at Earth's radius) SHALL report its wall time on the reference machine against terrain's world-build share (BUDG-014), with no time limit of its own, measured by the perf harness, never in a test (CON-20). It SHALL stay within the resident memory budget (RAM no more than 8 GB, CON-21), which the tiled bake supports by never holding more than a batch of blocks (tested at tier A under GEN-013). The `global-bake` perf scene (`perf_global_bake`) cites this requirement for spec-lint (TEST-001). The harness entry is `cargo xtask perf global-bake` (ADR 0020 gate, default 16384 cells per face edge, log `perf/logs/global-bake.json`: wall time, most blocks alive and their bytes, store size).

Verify: C
Status: active
Source: architecture report §14 (M3 exit criteria), CON-20, CON-21; ADR 0097 point 3 (the ten-minute limit retired, owner 2026-11-03)

#### Scenario: Bake timing
- **WHEN** the field bake of a 1 km grid runs on the reference machine in a perf window
- **THEN** its wall time is reported with terrain's world-build share, and its resident memory stays within the budget

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

#### Scenario: One height whichever tile asks
- **WHEN** the heights of the directions of a tile (its grid and halo) are asked for through that tile, a neighbour, its parent, its child and a tile on the far side of the planet, and through meshes of the tile at 8 and at 16 cells
- **THEN** every ask gives the same bits for a direction, and points that two neighbouring tiles share (the same bits of direction), also across a face edge or the cube corner, get the same height from either tile

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
The tile builder SHALL produce, for levels 3 and finer, a compact tile (ADR 0033 point 4): one 8-byte texel per grid vertex (`(cells + 1)^2` texels: a 32-bit height step count between the tile's own minimum and maximum, then in the second word two signed 15-bit octahedral normal components in bits 0-14 and 15-29, bits 30-31 zero), the tile's climate block (GEN-021: 81 words, 324 bytes), its driver block (GEN-074: 81 words, 324 bytes), its channel block (GEN-126: 81 words, 324 bytes) and a 128-byte header (nine tile-local control points of a biquadratic base surface on the shell through the tile origin, the height range `hr_min` and `hr_step`, the origin height, the skirt depth and the flags: bit 0 the orientation flip, every other bit zero), laid out as one slot of 9,812 bytes at cells 32 (GEN-025; `MESH_VERSION` 11). A compact tile keeps the minimum of 16 cells per tile edge (a build with fewer cells is refused naming the minimum). A tile's key SHALL be the mesh key of its tile (GEN-010). The canonical normal of a vertex (ADR 0073) SHALL be the cross product of the centred differences of the positions of its lattice neighbours, `n = (q(k+1, l) - q(k-1, l)) x (q(k, l+1) - q(k, l-1))` normalised with `q = d (R + h)` in f64, where `d` is the neighbour's unit direction and `h` its height; every face has `U x V` outward, so `n` is outward without a flip, and a flip is an assertion failure (except at the eight cube corners, below). The neighbours are the tile's own lattice points and a one-sample ring around them (without its four corners); a ring point across a tile edge inside a face is the next tile's lattice point, and one beyond a face edge is the neighbour tile's own lattice direction one cell inside it, on the row that continues the shared edge, found from the exact integer cube point and never by projecting the extended plane back (`TileId::ring_direction`). Two tiles sharing a vertex therefore read the same directions and compute bit-identical normals, inside a face and across the twelve cube edges (a cross product negates exactly when both operands or their order change sign). At a cube corner the three tiles read the same three edge points, order them canonically by the pair of faces that hold each (`TileId::corner_stencil`), form `(p1 - p0) x (p2 - p0)` from the same operands and orient it outward; their normals are bit-identical. The CPU mesh and the compact tile use this one definition. The stored normal SHALL equal the canonical CPU normal within 0.01 degrees (CON-14); the height SHALL be reconstructed as `hr_min + f32(q) * hr_step` with the step the relief over 16,777,215 (2^24 - 1 levels in the 32-bit field, so every `q` is an integer an f32 holds exactly and the CPU and the shader form the same `f32(q)`) and the range rounded outwards so no height is clamped, the height error SHALL be at most half a step plus f32 rounding of the sum (the step is at most 1 mm up to 1 mm x 16,777,215 = 16.7 km of relief in one tile; the f32 sum `hr_min + f32(q) * hr_step` adds about 2^-24 of the relief and of `|hr_min|`, so 1 mm of reconstruction error holds up to roughly 8 km of relief; the field stays 32 bits wide), and every tile SHALL be built compact whatever its relief (ADR 0062: there is no vertex fallback; the wide texel holds CON-13's 1 mm on steep finest tiles, SPIKE-12). The bound of the wide texel is the relief of one tile: the step is relief / 16,777,215, so it is at most 1 mm up to 16.7 km of relief in one tile and grows in proportion above (about 1.19 mm at 20 km, 2.4 mm at 40 km). The builder SHALL NOT refuse a tile for its relief: no spec carries a level-relevant tolerance that a larger step would break, so the bound is documented and tested (16.7 km and 20 km), not enforced. Tiles of levels 0-2 are not drawn (LOD-011) and not stored (STRM-021). The CPU mirror of the shader's vertex stage (position, morph target, skirts, heights, normal) SHALL agree with the canonical CPU mesh at every vertex of tiles of levels 3 to 26 within the model error, the quantisation step and f32 rounding, with the exact morph target (neighbour heights read from the texels), and neighbouring tiles of one level SHALL meet within the model error of the two (millimetres from level 12). Tiles SHALL be bit-identical across runs and thread counts, with known-answer hashes next to the mesh version they belong to. Every value in a tile SHALL be tile-local or a unit vector (CON-11).

Verify: A, B, C
Status: active
Source: ADR 0033 points 4 and 5, SPIKE-06, CON-11, CON-13, CON-14, LOD-010, GEN-025, ADR 0062, SPIKE-12

#### Scenario: Layout and bytes
- **WHEN** a tile is built at cells 32 and cells 64
- **THEN** it has 1,089 / 4,225 texels of 8 bytes, a 128-byte header and a 324-byte climate block and a 324-byte driver block and a 324-byte channel block (slots of 9,812 B and 34,900 B, GEN-025), no texel carries climate, and its key equals the mesh key of the same tile under `MESH_VERSION` 11

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
A compact tile SHALL be one slot of contiguous bytes, the same in the CPU tile, the pack payload (STRM-021), the pool block (REND-016) and the shader's reads (ADR 0062 point 2). The stride SHALL be `1,100 + 8 (cells + 1)^2` bytes, a multiple of 4 for every even cell count, with these offsets as constants of the layout version, and `MESH_VERSION` SHALL be 11 (payload version 4):

| offset | bytes | content |
|---|---|---|
| 0 | 128 | header: nine control points, `hr_min`, `hr_step`, origin height, skirt depth, flags |
| 128 | 324 | climate block (GEN-021) |
| 452 | 324 | driver block (GEN-074) |
| 776 | 324 | channel block (GEN-126) |
| 1100 | 8 (cells + 1)^2 | texels (GEN-019), row-major |

The slot SHALL be bit-identical across runs, thread counts and operating systems, with every value tile-local or a unit vector (CON-11).

Verify: A, B
Status: active
Source: ADR 0062 point 2, ADR 0063, ADR 0076, SPIKE-12 Q3, ADR 0033 points 3-4, GEN-019, GEN-021, GEN-074, GEN-126, CON-11, CON-14

#### Scenario: Offsets and stride
- **WHEN** a slot is built at cells 16, 32 and 64
- **THEN** the strides are 3,412, 9,812 and 34,900 bytes (8,712 B of texels at cells 32), every stride is a multiple of 4 and the shader reads the slot only as `u32` words from a block bound whole (REND-016), and the header, climate block, driver block, channel block and texels sit at the offsets of the table

#### Scenario: One byte string
- **WHEN** a tile's slot is encoded to the pack payload, decoded, uploaded to a block and read back
- **THEN** the four byte strings are equal and equal the CPU builder's, and the shader's reads of header, climate, driver and texel words (the channel block is not read yet) at the offsets agree with the CPU mirror

#### Scenario: Determinism and version
- **WHEN** the slots of 24 tiles are built on 1, 2 and 4 threads
- **THEN** the content hashes are equal to each other and to the committed known answers of `MESH_VERSION` 11; the channel block moved the texels from byte 776 to 1,100 and each key and persistent key changes with the version

### Requirement: GEN-026 The stamp stage
A stamp stage SHALL add the stamps of one layer of a world definition to a base height field. A stamp has an anchor, a reach `R` (its `reach_m`, above 0), a bound `M` (its `max_delta_m`), a kind and parameters in metres; with `d` the arc distance from the anchor to the point on the body's reference sphere, computed in f64 with `libm` as `radius * atan2(|a x p|, a . p)`, and `x = d / R`, its delta SHALL be zero for `x >= 1`, C1 at `x = 1`, and never exceed `M` in absolute value. A `hill` SHALL add `amount * (1 - x^2)^2`. A `crater` SHALL add `amount * (-bowl(x) + 0.2 * rim(x))` with `amount` its depth (at least 0), `bowl(x) = 1 - s(min(x / 0.8, 1))`, `rim(x) = (1 - ((x - 0.85) / 0.15)^2)^2` for `0.7 <= x <= 1` and 0 elsewhere, and `s(t) = t^2 (3 - 2t)`. A `plateau` SHALL add `w(x) * clamp(target - h, -M, M)` with `target` an absolute height above the reference sphere, `h` the running height at the point and `w(x) = 1 - s(clamp((x - 0.7) / 0.3, 0, 1))`; it reads nothing at its anchor. Stamps SHALL apply in application order (priority, then id), each to the running height. The stage's height at a point SHALL be the base height plus the stamps whose reach contains it, SHALL depend on nothing but the base heights, the stamps of the layer and the point, and SHALL be identical at any thread count, batch order and batch size (GEN-020); it fetches its items once per batch through a reader, so a point is a batch of one. The stage's `height_bounds` for a cap SHALL be the range of the running height at the points of the cap: the base bounds carried through the items whose reach touches the cap, in application order, in f64 with outward rounding. A hill and the bowl of a crater widen the range by the range of their delta, which is monotone in `x` (taken at the nearest and the farthest distance of the cap from the anchor; the crater's profile rises to the rim's crest at `x = 0.85` and falls beyond it, so its range is that of the two end values and, where the cap holds `x = 0.85`, the crest's). A plateau maps the range: `h + w clamp(target - h, -M, M)` is non-decreasing in `h` and linear in `w`, so the image of the range is taken from the ends of the range and the ends of the weight over the cap (`w` at the farthest and at the nearest distance). Where the weight is 1 over the cap the image is the range of the gap to the target; the declared bound `M` limits it and is never added on both sides. A cap on the flat top of a hill or of a plateau therefore grows by centimetres or by the extent of the base there, not by `M`, and planning finds no empty morph interval where the base found none, whatever the declared bounds of the items. A new stamp kind SHALL have a profile monotone in `x` on each part or a bounds rule of its own. The widening is done in the stage only. The stage SHALL delegate everything else (body, seed, surface, climate, water cull level) to the base, SHALL read the base only as a height field at points, and SHALL declare its items for a tile with the halo `STAGE_HALO` = 0.25 of the tile edge.

Verify: A, C
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

#### Scenario: Reads contract over the authored stack
- **WHEN** the stack of stamp, line and brush stages generates tiles from continental to metre scale, near the items and far from them, through a recording reader
- **THEN** every item read is in the footprint its tile declared, removing the items the tile did not declare changes none of its heights (nor the key of a tile of level 11 or finer), and removing an item that was read re-keys the tile

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
Chunks SHALL lie on a fixed grid of the tiles of level 11 (whole coarse cells, about 5 km), and the lattice of their cells SHALL state its raster cell size; its apply level SHALL be the coarsest tile level whose vertex spacing is at most twice that cell size (level 15 for 8 m cells, level 16 for 4 m), computed from the raster and never set by hand. The cells of a chunk SHALL be the cells of its tile, and the chunks a tile reads SHALL be listed own chunk first, then the neighbours its reads reach, in a fixed order.

Verify: A
Status: active
Source: ADR 0077 point 5, SPIKE-34 (conditions 1 and 3), STRM-029

#### Scenario: Apply level
- **WHEN** a chunk lattice has 8 m cells, then 4 m cells
- **THEN** its apply level is 15, then 16

#### Scenario: Cells and reads
- **WHEN** cells of chunks on the middle of a face, on a face edge and at a cube corner are placed, and the chunks read by a tile are listed
- **THEN** each cell lies in the tile of its chunk and holds its own centre, and the list starts with the tile's own chunk and follows in a fixed order

### Requirement: GEN-039 The chunk stage's position per layer
A layer SHALL have a `chunk_stage` setting, `Under` by default; `OnTop` SHALL stay representable and enter the definition hash (WDEF-018). The setting SHALL act on no build (ADR 0086): the stamps of a layer stack in the order of ADR 0007 whatever it says.

Verify: A
Status: active
Source: M8 plan D3 (owner, 2026-10-26), ADR 0077 point 7, m8-chunk-version, ADR 0086

#### Scenario: Default and refusal
- **WHEN** a world with no setting is built, and then with a layer set to `OnTop`
- **THEN** the layer is `Under` in the first, `OnTop` in the second (nothing is refused), and the layers stack in the order of ADR 0007 in both

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

Detail: `generators::coarsehorizon` SHALL compute, from the stored heights of the coupled run (STRM-043), the coarse horizon of the cells of an observer lattice of `m` cells per face edge (a divisor of the run's): per cell and per azimuth the elevation angle (the chord above the observer's horizontal plane, on the sphere) of the highest sample of the run along the great circle in that azimuth, out to the far range (default 50 km, 16 azimuths, samples one cell of the run apart, at most 256 azimuths, a range of at most a quarter of the way round the body), where each sample is the run's height at the sample's direction (so a march crosses a cube face edge without face coordinates) and sea at or below 0 counts as 0; the observer stands at the lowest and at the highest of the run cells its cell covers, so a cell stores two horizons and its lowest and highest ground in steps of 40 m (8 bits; a cell is `2 a + 2` bytes for `a` azimuths, `record_len`), and a point's horizon is the blend of the two by its height between those heights (`blend_elevation`): a crest above the cell's mean ground is not shadowed by a plateau its own height clears (owner test 2026-10-27, frame-102263: the one horizon of the cell's mean ground blanked 20 to 30 km of lit crests under a sun below about 1.8 degrees). Stated limits of the blend: it lies at or above the exact horizon between the two heights (more shadow, never less: the exact horizon is convex in the observer's height; up to about 1.7 degrees in the plateau test), the heights are stored up to 10,200 m, and the shader blends after the bilinear filter of the four cells, not before. The angle is the byte of GEN-044. Azimuths are in the geographic frame (0 toward the pole axis, a quarter turn east), unlike GEN-044's raster frame; GEN-046 rotates the sun's azimuth into each. The result is independent of the thread count, and a wrong size, a bad `m` or parameters that are not finite and positive are refused with the cause named. The result SHALL be a pure function of the stored heights and the parameters and be stored as a global channel beside the others (STRM-045). Stated limit: each cell is baked in the geographic frame of its own centre while the lookup (GEN-046) uses the fragment's own geographic frame, so within about 5 coarse cells of a pole (about 100 km at 512 cells) the frames differ by up to the longitude difference and the horizon read at an azimuth can come from another bearing; the fix is a pole-free frame such as the face raster frame.

#### Scenario: A crest in a cell below a plateau
- **WHEN** a cell whose mean ground is below a plateau sunward has one crest of 1,500 m above the plateau, and the sun is 0.9 degrees up
- **THEN** the horizon from the cell's mean ground is over 1 degree, from its low ground over 1 degree, from its crest under 0.4 degrees, a point on the crest is lit and a point on the low ground is in shadow

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

### Requirement: GEN-071 Driver channels of a tile and their blend
The driver block of a compact tile (GEN-074) SHALL hold the soil depth, the moisture and the insolation as three 8-bit codes per texel. A consumer SHALL read a driver as `neutral + w (value - neutral)` with `w` the weight of the block's value (1 in the fragment stage); the neutral values are soil 1 m, moisture 0.5 and the insolation of the latitude alone.

Verify: A
Status: active
Source: ADR 0076, planner decision 2026-10-26, ADR 0086

#### Scenario: The range edge
- **WHEN** the weight goes from 0 to 1
- **THEN** the driver goes from the neutral value to the block's value, monotone, with no step

### Requirement: GEN-072 The materials read the drivers
`MaterialParams::weights` SHALL read the soil depth, the moisture and the insolation from the tile's driver block in its reserved inputs (and the fill depth and the erosion depth from the channel block in its two, GEN-126), by the same function and the same input vector as for the other inputs.

Verify: A
Status: active
Source: LOOK-008, ADR 0076 point 5

#### Scenario: A hollow
- **WHEN** a look gives soil a curve on the moisture input and the drivers describe a wet hollow
- **THEN** soil takes weight in the hollow and none on the dry flank

### Requirement: GEN-073 The default look ignores the drivers
The default look SHALL give no curve to the soil-depth, moisture, insolation, fill-depth and erosion-depth inputs, so its material weights and albedo SHALL be bit-identical for every driver value, and the images of every committed golden SHALL be unchanged with no candidate; the neutral values matter only once a look gives those inputs curves.

Verify: A, C
Status: active
Source: planner decision 2026-10-26, ADR 0017

#### Scenario: Today's goldens
- **WHEN** the default look's weights are evaluated for several driver vectors and the committed goldens are rendered
- **THEN** the weights are equal bit for bit and every image equals its golden

### Requirement: GEN-074 The tile driver block and the driver views
A compact tile SHALL carry a driver block of 9 x 9 words (GEN-025): at each climate texel the neutral soil depth, moisture and insolation codes, and the neutral insolation of the texel's latitude in the top byte (neutral unless the field reads baked channels, GEN-119). The fragment stage SHALL read the block bilinearly, blend it as `neutral + w (value - neutral)` with `w` 1 (GEN-071), and fill the reserved material inputs (GEN-072). The debug views `soil`, `moisture` and `insolation` (`ShadeView` 7 to 9, reached by J) SHALL show the blended drivers in false colour.

Verify: A, C
Status: active
Source: ADR 0076 point 5, GEN-071, GEN-072, GEN-025, ADR 0086, GEN-119

#### Scenario: The neutral block
- **WHEN** compact tiles are built over a field
- **THEN** every texel of the block holds the neutral codes of its latitude

#### Scenario: The views
- **WHEN** a compact tile whose driver block is uniform, and one whose block is a ramp, are drawn in each driver view
- **THEN** every covered pixel has the colour the CPU mirror gives for those codes (within 0.003 for the uniform block), and a node with no fade plane reads the block's values at full weight

#### Scenario: Constants
- **WHEN** the block is decoded
- **THEN** the soil code uses the default `soil_max_m` of 8 m in the builder and the shader alike

### Requirement: GEN-079 A compact batch takes its footprint from a hint
A batch of directions that lies on one face at least `reach + 2` cells from its edges, in a box of at most 4,194,304 cells, evaluated by a field whose reads stay within `reach` cells of a direction's cell (the eroded field's `reach` is 8), SHALL take the cells to gather from the box of `reach` cells before and `reach + 1` after the cell of each direction, not from a pass that reads for every direction; any other batch SHALL take the exact footprint. The gathered cells are the source's own samples, so every value is bit for bit the single `height` at any thread count, with the tile's climate (GEN-021) joined.

Verify: A
Status: active
Source: opus-planner (2026-10-26), GEN-020, perf window 2026-10-26

#### Scenario: Bits and reads
- **WHEN** compact batches (across a block edge, inside a block, near a face edge, at a cube corner) are evaluated on 1 and 8 threads
- **THEN** every value equals the single `height`, no read fails, and a batch of about 12 cells reads fewer than 2,000 cells and decodes the four blocks round its centre once

### Requirement: GEN-043 The coupled run through the climate views
`generators::coupled` SHALL draw the stored run onto a tile's climate texture (9 x 9 texels, GEN-021) so the viewer's climate views show the global river network: with the height lens the temperature channel holds the run's height (sea at -30 C, land from the height grey of the height debug view up to 40 C) and the precipitation channel is 0; with the area lens the precipitation channel holds the drainage area on the log scale of the area map (0 mm at one cell's area, 3000 mm at the largest area of the run) and the temperature channel is -30 C.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-1 part c), ADR 0077

#### Detail
A texel's height is the run's at its cell; its area is the largest in the cells the texel covers, so a river a coarse tile cannot resolve still shows. `generators::chain::CoupledLensField` SHALL put the lens over any field: every height, key and reading is the base's, the climate is the lens's, and the climate version word (hence the mesh key of every tile) carries the run's identifier, the lens and the run's algorithm version. The texture is a pure function of the run, the lens and the tile. The lens's read records and its answer to whether it reads cells SHALL be those of the layer the key walk reaches below it at the tile (`key_source`, STRM-029), so a tile a stage passes through keys like the base's, never panics the key.

#### Scenario: What a tile shows
- **WHEN** the lenses are applied to the tiles of a run on a small lattice, to the whole face as one tile and to a fine tile
- **THEN** the height lens holds sea at -30 C and land above it, the area lens holds the largest area of a texel's cells (the main stem on the wet half of the scale from the coarse tile), the other channel is flat, and two draws are equal

#### Scenario: The lens field
- **WHEN** a field is wrapped in the lens
- **THEN** its heights are equal to the base's bit for bit, its climate is the lens's, and its mesh key differs from the base's and between the two lenses and between two runs

### Requirement: GEN-056 The coupled run's lattice is independent of the bake's
`generators::coupledinput::reduce_bake` SHALL give the run's starting heights on a lattice of `n` cells per face edge from a bake of `res` cells per face edge, `n` dividing `res` (anything else refused with both numbers). A bake cell holds the mean of its four corner vertices and a run cell the mean of the `(res / n)^2` bake cells it covers, summed in `f64` in a fixed order and rounded to `f32` once, so the result is bit-equal for any thread count and block structure of the source (CON-14). The run SHALL start from the reduced heights exactly as from the same heights given directly. The world's run SHALL use `min(res, 2048)` cells per face edge.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-1 (c) rest, owner: solid software), ADR 0077, CON-14

#### Scenario: Reduction and run
- **WHEN** synthetic bakes of 16 to 64 cells are reduced to runs of 1 to 16 cells per face edge, with 1 to 64 threads and blocks of 0 to 64 cells, and a run of 8 cells is made from bakes of 32 and 64 and from the heights reduced by the definition
- **THEN** every run cell equals the definition's mean, a plane reduces to the plane at the cell centre, all thread counts agree bit for bit, the two runs agree bit for bit in heights, area and receivers, a run that does not divide the bake is refused and a missing vertex is named

### Requirement: GEN-042 Global coupled run time
The global coupled run (GEN-040) at 2048 cells per face edge and 100 steps SHALL take no more than ten minutes on the reference machine, measured by the performance harness through the architect and never in a test (CON-20). The `coupled` perf scene (`perf_coupled`) cites this requirement for spec-lint (TEST-001). The harness entry is `cargo xtask perf coupled` (ADR 0020 readiness gate, log `perf/logs/coupled.json` with wall time, steps, seconds per step and threads); the run is single-threaded, so `threads` is 1.

Verify: C
Status: active
Source: opus-planner (2026-10-24), M8 exit criterion 4

#### Scenario: Coupled run timing
- **WHEN** the coupled run of 2048 cells per face edge and 100 steps runs on the reference machine in a perf window
- **THEN** it finishes within ten minutes

### Requirement: GEN-088 The coupled run is timed from a stored bake
`cargo xtask perf coupled --store DIR --global-res N --remake-into OUT` SHALL time a fresh make of the coupled run from the bake stored in `DIR` (the viewer's make: the reduction of the bake, then the steps; any stored channel is ignored) against the ten minutes of GEN-042, and write `perf/logs/coupled-make.json`. `OUT` SHALL be created if missing, SHALL NOT be inside `DIR` and SHALL lie under a `target` directory (the store is only read).

Verify: A, C
Status: active
Source: opus-planner (2026-10-27), M8 exit criterion 4

#### Scenario: The output directory
- **WHEN** `OUT` is the store, a directory inside it, or a directory with no `target` component
- **THEN** the make is refused naming the store

### Requirement: GEN-064 The hill band and the landform driver of the bake
`global::sample` SHALL add to the elevation of land a hill band: a ridged multifractal of three octaves at the frequencies 796, 1592 and 3184 per unit direction (wavelengths of 8, 4 and 2 km at Earth's radius; they scale with the body's radius) on the warped direction (which stretches them locally), weights 1, 1/2 and 1/4 of `(1 - |n|)^2`, normalised, times an amplitude and times `smoothstep(0, 0.1, land depth)`. The band SHALL be 0 over the sea and at the coast and never negative, and at most `HILL_BAND_MAX_M` (1000 m).

Verify: A
Status: active
Source: owner (2026-10-25, via opus-planner): plausible relief in M8; ADR 0080 points 1, 3 and 5, `openspec/changes/m8-relief/proposal.md`

#### Detail
The amplitude SHALL be `30 + 145 smoothstep(0, 0.25, m) + 825 smoothstep(0.25, 0.7, m)` m for the mountain strength `m = landform x (0.25 + 0.75 ridge)`. `global::landform(seed, d)` SHALL be the range-zone strength times the distance from the coarse coast (`smoothstep(0.02, 0.35, depth)` of the coarse continent value of GEN-104, whose rough full coast the driver's lattice could not hold; the driver is 0 at the coarse coast, not exactly at the full value's), in `[0, 1]`, a pure function of the seed and the direction that does not read the band. The bake's elevation clamp and the field's `max_abs_height_m` SHALL stay as they are while the highest base and range (6200 m) plus `HILL_BAND_MAX_M` stay below the clamp. The hill band's mountain strength SHALL read the same driver. `GLOBAL_VERSION` SHALL be 6 (the continent stage, GEN-104).

#### Scenario: Where the band is and how large
- **WHEN** 40,000 directions spread over the sphere are sampled
- **THEN** the band is between 0 and 1000 m, 0 over the sea, positive over a quarter of the land at least, the elevation never exceeds the clamp, and the highest base plus range plus the band is below it

#### Scenario: By landform
- **WHEN** land with landform under 0.02 and land with landform over 0.5 are compared
- **THEN** the plains carry at most 40 m and the mean band of the ranges is at least eight times the plains'

#### Scenario: Gentle at 100 m
- **WHEN** the band is stepped 100 m along 300 great circles
- **THEN** the largest step is under 90 m (58 m recorded; the warp stretches the octaves locally, so this is a regression guard and not a bound: a 1 km octave or a doubled frequency would move it by far more)

#### Scenario: Known answers
- **WHEN** the landform driver is read at four fixed directions
- **THEN** its bits are the pinned ones (GLOBAL_VERSION 5)

### Requirement: GEN-063 The stationary uplift of the coupled run
`coupled::stationary_uplift` SHALL give each land cell the uplift `rho K A^m S` in m/year, `A` the drainage area and `S = (z - max(z_r, 0)) / distance` the slope to the receiver, both taken from the input heights `z` by the run's priority flood, jittered receivers and accumulation, so that the implicit update `z = (z + dt U + f z_r) / (1 + f)` leaves the input where it is at `rho` 1 (to a few f32 ulps; not the cells in a depression, which the flood raises, nor land cells under `min_height_m`, which the step lifts to it, nor the cells that drain to either, which read the moved height).

Verify: A
Status: active
Source: owner (2026-10-25, via opus-planner): plausible relief in M8; ADR 0080 point 4, `openspec/changes/m8-relief/proposal.md` items 5 and 6, GEN-040

#### Detail
Sea cells, outlets and cells whose drop to the receiver is not positive SHALL get 0, the result SHALL be linear in `rho` (a factor of zero or more) and a pure function of its arguments in the fixed order of the run (CON-14). The coupled run of a world SHALL, when it erodes (`CoupledParams::erode`, off for M8, GEN-065), take this field with `rho` `UPLIFT_RHO` (1) in place of a constant uplift; `rho` and `UPLIFT_FIELD_VERSION` SHALL be part of the declared identity of the run's inputs (STRM-043).

#### Scenario: The input is kept
- **WHEN** a 24-cell world runs 20 steps on its stationary uplift at `rho` 1 and, from the same heights, on a constant uplift of 1e-3 m/year
- **THEN** the median height change over land is under a tenth of the constant run's and under 20 m, one step moves every land cell that is not in a depression, under the height floor or draining to such a cell by at most 4 f32 ulps (coastal cells included), the uplift is never negative, `rho` 0 gives zero everywhere and `rho` 2 gives exactly twice the field, and in a world without sea the outlets and the cells in pits get 0

#### Scenario: Sea and determinism
- **WHEN** the field is made twice on a 12-cell world
- **THEN** both agree bit for bit and every sea cell is 0

### Requirement: GEN-065 The coupled run keeps the relief of its input
`CoupledParams::erode` SHALL select what the run does and SHALL be part of the key of the stored run (STRM-043). With it off, `run` SHALL return `coupled::filled_state`: the heights are the priority-flood filled surface of the input (a cell outside a depression keeps its input height bit for bit, a cell in one takes the level of the fill, the sea is untouched), the receivers are those of the jittered steepest descent on that surface and the drainage areas their accumulation (every receiver lower than its cell, the outlets draining the land area), in one pass, in the fixed order of the run, with `libm` (CON-14).

Verify: A
Status: active
Source: planner (2026-10-25, after the rebuild: the stationary uplift did not hold the input on the real bake), ADR 0080 point 4, GEN-040, GEN-063

#### Detail
A world's run SHALL have it off (`coupledworld::COUPLED_ERODE`) and SHALL not make the stationary uplift of GEN-063 then. With it on, the stream-power steps of GEN-040 SHALL run as they did.

#### Scenario: A real reduced bake
- **WHEN** the global-coarse field baked at 256 cells and reduced to 64 per face edge runs without erosion for 80 steps
- **THEN** the heights are the filled surface, every cell outside a depression has its input height bit for bit and the median of |run - input| over land above 1000 m is 0, while the eroding run of the same input moves it by over 100 m (1,175 m measured), and the receivers are downhill and the outlets drain the land

#### Scenario: The key
- **WHEN** two specs differ only in `erode`
- **THEN** their keys differ

### Requirement: GEN-085 The horizon march walks a step table
`generators::horizon::bake_horizon` SHALL make the distance of every step of the march and its curvature drop `d^2 / (2 R)` once per bake and walk that table for every texel and azimuth. The table's values and the march's arithmetic SHALL be the ones of a march that adds its steps as it goes, so the bytes of a map are those of GEN-044 before this requirement.

Verify: A
Status: active
Source: opus-planner (2026-10-26, criterion 6), GEN-044, CON-14

#### Scenario: Equal to the long form
- **WHEN** a rough raster is baked at three ranges, two growths and three body radii
- **THEN** every byte equals that of a march that forms each distance and drop as it goes

### Requirement: GEN-086 The horizon march has an AVX2 kernel with the bytes of the scalar one
`generators::horizon` SHALL march four azimuths of a texel at once on a CPU with AVX2, chosen at run time, and SHALL give for every raster, block, parameter set and thread count the bytes of the scalar march (GEN-085), which every other machine and every remainder azimuth runs.

Verify: A
Status: active
Source: opus-planner (2026-10-27, owner: no deferred optimisation), GEN-044, GEN-085, CON-14

Detail: The kernel SHALL do the scalar march's IEEE operations on each lane in the same order, with no fused multiply-add, and SHALL treat a sample outside the raster as the scalar march does (the lane stops). Both kernels SHALL be tested against each other on CI wherever AVX2 is present, with azimuth counts of 16, 7, 4 and 1, with texels whose rays leave the raster, and with a raster of NaN heights.

#### Scenario: Both kernels
- **WHEN** a rough raster is baked on the scalar kernel and on the AVX2 kernel
- **THEN** every byte is equal, for texels near the raster edge as well

### Requirement: GEN-089 Local terrain descriptors
`planet_generators::descriptors` SHALL compute, on a height raster of cell size `h` with marked no-data cells (NaN), the slope in degrees (`atan` of the central-difference gradient) and the profile and contour curvature in 1/m from the Evans-Young quadric fit over the 3x3 window. Profile curvature is taken along the gradient and contour curvature across it, both positive where the surface curves upward (a bowl). The profile is 0 where the gradient is under 1e-6 and the contour is uncounted where it is under 1e-3 (it would only report noise). A cell whose window touches the edge or a no-data cell SHALL be uncounted. The computation SHALL use f64 and `libm` and give the same bytes at any thread count (CON-14).

Verify: A
Status: active
Source: SPIKE-57 (scorecard, descriptors survey CGF 44(2) 2025), SPIKE-55, CON-14

#### Scenario: Definition
- **WHEN** the window is `z1 z2 z3 / z4 z5 z6 / z7 z8 z9` (row `j - 1` first, `i` along x, `j` along y)
- **THEN** the gradient is `p = (z3+z6+z9-z1-z4-z7)/6h` and `q = (z7+z8+z9-z1-z2-z3)/6h`, the slope uses `(z6-z4)/2h` and `(z8-z2)/2h`, and with `r = (z1+z3+z4+z6+z7+z9-2(z2+z5+z8))/3h^2`, `t = (z1+z2+z3+z7+z8+z9-2(z4+z5+z6))/3h^2` and `s = (z9+z1-z3-z7)/4h^2` the profile curvature is `(p^2 r + 2pq s + q^2 t) / ((p^2+q^2)(1+p^2+q^2)^(3/2))` and the contour curvature `(q^2 r - 2pq s + p^2 t) / (p^2+q^2)^(3/2)`

#### Scenario: Plane and quadrics
- **WHEN** the kernels run on a plane rising 10 degrees east, and on `z = a (x^2 + y^2)` and `z = a x^2` sampled at `(x0, 0)`
- **THEN** the slope is 10 degrees at every counted cell within 1e-9, the profile curvature is `2a / (1 + 4 a^2 x0^2)^(3/2)` on both within 1 %, and the contour curvature is `1 / x0` on the paraboloid and 0 on the trough, also off the axis at `(x0, y0)` with `1/rho` and `rho^2 = x0^2 + y0^2` within 1e-9

#### Scenario: Flat ground
- **WHEN** the kernels run on a plane with 1e-4 m of noise at 30 m cells
- **THEN** every contour curvature is uncounted and the profile curvature stays bounded

#### Scenario: Threads
- **WHEN** the kernels run on the same raster with 1 and 8 threads
- **THEN** the outputs are byte-identical

### Requirement: GEN-090 Relief and position descriptors
The descriptors module SHALL compute the local relief (max minus min) over non-overlapping square blocks of a given side, and the topographic position index (TPI): the height minus the mean over the square window of side `2r+1` cells, from summed-area tables accumulated in f64 in row order. A window that touches the edge or a no-data cell SHALL be uncounted. The results SHALL be thread-count independent (CON-14).

Verify: A
Status: active
Source: SPIKE-57, SPIKE-55 (relief by blocks), CON-14

#### Scenario: Edges
- **WHEN** blocks and windows meet the raster edge or a no-data cell
- **THEN** blocks start at the origin and a partial block at the east or south edge is dropped, a block holding a no-data cell has relief NaN, and the TPI is counted only at data cells at least `r` from the edge whose window holds no no-data cell (a second summed-area table counts the data cells; both tables are built on one thread by row prefix sums)

#### Scenario: Step and wave
- **WHEN** a raster holds a 100 m step through one block and a long sine wave
- **THEN** that block's relief is 100 m and a flat block's is 0, the TPI is positive on the crests and negative in the troughs, and the summed-area mean equals the direct mean within 1e-9

### Requirement: GEN-091 Geomorphons
The descriptors module SHALL classify each counted cell into one of ten landform classes (flat, peak, ridge, shoulder, spur, slope, hollow, footslope, valley, pit) after Jasiewicz and Stepinski (2013): in 8 directions out to a lookup distance, the zenith and nadir angles against a flatness threshold give a ternary pattern, and the counts of lower and higher directions select the class by the table of the reference implementation. Defaults are a lookup distance of 1 km (33 cells at 30 m) and a threshold of 1 degree. A cell closer than the lookup distance to the edge, or whose lines hold a no-data cell, SHALL be uncounted. The results SHALL be thread-count independent (CON-14).

Verify: A
Status: active
Source: SPIKE-57, CON-14

#### Scenario: The pattern
- **WHEN** a direction has the highest elevation angle (zenith) and the lowest (nadir) over the lookup cells, diagonal steps being `h sqrt(2)` long
- **THEN** after the rule `compare_multi` of GRASS `r.geomorphon` (`pattern.c`, mode anglev2), the direction is level (0) when neither angle's size exceeds the threshold, higher (+) when only the zenith's does, lower (-) when only the nadir's does, and when both do the larger size wins and an exact tie is higher; a 0.7 degree plane at a 1 degree threshold is level in every direction

#### Scenario: The class table
- **WHEN** `m` directions are lower and `p` are higher (`m + p <= 8`, 45 pairs)
- **THEN** the class is the entry `forms[m][p]` of `determine_form` in GRASS `r.geomorphon` (`geom.c`), listed by `m` (`p` from 0): 0: FL FL FL FS FS VL VL VL PT; 1: FL FL FS FS FS VL VL VL; 2: FL SH SL SL HL HL VL; 3: SH SH SL SL SL HL; 4: SH SH SP SL SL; 5: RI RI SP SP; 6: RI RI RI; 7: RI RI; 8: PK (FL flat, PK peak, RI ridge, SH shoulder, SP spur, SL slope, HL hollow, FS footslope, VL valley, PT pit)

#### Scenario: Forms
- **WHEN** the classifier runs on a cone, an inverted cone, a V valley, a ridge and a tilted plane
- **THEN** the apex is a peak, the inverted apex a pit, the valley axis a valley, the ridge axis a ridge, and the plane slope (flat below the threshold)

### Requirement: GEN-092 Drainage descriptors
The descriptors module SHALL fill depressions by a priority flood with an epsilon of 1e-3 m, ties broken by cell index. It SHALL then route D8 (steepest descent, ties by direction order), accumulate the drainage area in topological order, and give the drainage density: the length of the channel cells (area at least a threshold, default 1e5 m2) over the counted area, in km per km2. The results SHALL be thread-count independent (CON-14).

Verify: A
Status: active
Source: SPIKE-57, SPIKE-40 (drainage density target), SPIKE-47, CON-14

#### Scenario: Definition
- **WHEN** a raster is filled and routed
- **THEN** a filled cell is `max(z, filled parent + 1e-3)`, the raster edge and the cells next to a no-data cell are the outlets, D8 compares the fall over `h` or `h sqrt(2)` on the filled surface and keeps the first of equal falls in the order N, NE, E, SE, S, SW, W, NW (`j` grows southward), a cell with no lower neighbour has no receiver, the area (own cell included) accumulates in reverse flood order on one thread (topological, as a receiver is strictly lower), a channel cell adds `h` (`h sqrt(2)` for a diagonal receiver) to the length, an outlet adds none, and the counted area is that of all data cells

#### Scenario: A tilted V valley with a pit
- **WHEN** the drainage runs on a tilted V valley holding one pit
- **THEN** the pit is filled, the area grows downstream along the axis by one cell row each step, and the density equals the analytic channel length over the area within 1 %

### Requirement: GEN-108 The descent site of a world
A world definition MAY name the viewer's descent site, `[world] site = [latitude, longitude]` in degrees (finite; latitude within 90, longitude within 180), written canonically and otherwise refused naming `world.site`. The site SHALL enter no hash. The viewer SHALL open the descent over the first of: the `--site` option; the definition's site; 20 N 30 E where the field there is land; the land 1 km inland of the first shore `find_coast` finds from 20 N 30 E; 20 N 30 E.

Verify: A
Status: active
Source: Owner test 2026-11-03 (the default site is sea on m9r2), inbox #52, #53

#### Scenario: The definition's site
- **WHEN** a definition with `world.site` is parsed, written and parsed again, and hashed with and without the site
- **THEN** the site round-trips exactly, the text is canonical, and the hash and base hash are equal

#### Scenario: A bad site
- **WHEN** `world.site` has one or three numbers, a string, a latitude beyond 90 or a longitude beyond 180 degrees, or a NaN
- **THEN** loading is refused, naming `world.site`

#### Scenario: The order of the sources
- **WHEN** the descent site is asked with `--site`, with a definition site, and with neither on worlds where 20 N 30 E is sea
- **THEN** the option wins, then the definition's, and otherwise the result is land (the field above the sea level there) and the same on a second call

### Requirement: GEN-109 The crease band
`with_crease_band(CreaseBand)` MAY drop a relief field's crease octaves above the erosion layer's grid (ADR 0098):
- it needs the relief channels on; without the call the field is today's, bit for bit;
- an octave of at most `near` layer cells (2) keeps its weight, one of `far` (4) or more is dropped, between them a smoothstep in the log of its cell;
- above the sea level it blends in over `blend_m` (200 m); below it the creases are today's;
- the bounds take the weighted octaves, and today's where a cap reaches the blend;
- the band enters the keys and the implementation id, not the field version.

Verify: A
Status: active
Source: ADR 0098 (owner, 2026-11-03), ADR 0088 points 2 and 5, GEN-099

#### Scenario: Off
- **WHEN** the field has the relief channels on and the call is not made, or the band keeps every octave, or its sea level is above every ground
- **THEN** every height equals today's field's to the bit, with and without the C1 base

#### Scenario: The weights
- **WHEN** the weight is read at 2 cells, 4 cells, their geometric mean and everything between and beyond
- **THEN** it is 1, 0 and 0.5, non-increasing, and the octaves of the Earth keep octaves 7 to 11, drop octaves 0 to 5 and weigh octave 6 below 0.02

#### Scenario: The sea floor, the land and the shore
- **WHEN** heights are read where the base is below the band's sea level, and along a line across a shore with a blend of 200 m and of 0
- **THEN** the first equal today's field's to the bit, most of the land's differ, and the largest step per half metre across the shore is a third of the hard step's or less, and under 3 m

#### Scenario: Bounds
- **WHEN** random caps, caps on cube edges and caps that reach the blend are bounded, and nested caps are compared
- **THEN** every height, also rounded to f32, lies within its bounds, the width of a cap on land is no more than today's, and an inner cap's bounds lie within the outer cap's, on both bases

#### Scenario: Keys
- **WHEN** the band's cell, cells, sea level or blend change, or it is switched on, and the relief channels are asked for after the band
- **THEN** the inputs hash and the implementation id differ, the late call is refused, and a batch and every thread count give the single reads' bits

### Requirement: GEN-119 The driver block from the baked channels
A field MAY give the tile builder baked channels at the 81 texels of a driver block (`HeightField::driver_channels`). The builder SHALL then write, at each texel that has them, the deposited depth as the soil code and the drainage area as the moisture code (a linear ramp of its log2 in cells, 0 at 0 and 1 at 10, `DRIVER_MAP_VERSION`); the insolation and a texel without a channel stay neutral. A field without channels gives the neutral block; every wrapper SHALL forward them. A field that reads channels SHALL fold `DRIVER_MAP_VERSION` and the channels' identity into its climate version word: its mesh keys differ from those without, no other field's key changes, and no mesh version bump (GEN-018).

Verify: A, C
Status: active
Source: planner inbox #65 (2026-11-05), ground-channels-2b decision 1, STRM-075, GEN-074, ADR 0076, ADR 0086

#### Scenario: Channels in the block
- **WHEN** compact tiles are built over a field that gives channels, over the same field without them, and over fields wrapping one, and the driver views of the tiles are drawn with the default look
- **THEN** each texel holds the soil and moisture codes of its channels with the insolation of the neutral block, the field without them has the neutral block and its old keys, the keys with channels differ from those without and between channel identities, every wrapper forwards both, the soil and moisture views show the channels, and the lit picture is the one it was

### Requirement: GEN-116 The lithology field
An analytic lithology field SHALL give, at a direction and a height, the erodibility K a carve reads, a caprock flag and a soluble flag: CPU-canonical (`libm`, integer hashing), never stored, the same bits for any thread count, keyed by `LITHOLOGY_VERSION`. Its hardness SHALL be SPIKE-81's formula of the class source's plate-history terms, read from a lattice of the class lattice's geometry, plus noise. Its strata SHALL be bands 30 to 170 m thick with a hardness offset from the province's, tilted and folded by a smooth offset of the height; K SHALL be continuous in the height, a band above hardness 0.92 a caprock, and a carbonate band soluble where the province allows.

Verify: A
Status: active
Source: ADR 0102, SPIKE-102 F1, SPIKE-81

#### Scenario: The field
- **WHEN** the field is built on 1 and 8 threads and read at random points and heights, its hardness is compared with SPIKE-81's formula, a column is scanned over 1.5 km in steps of a quarter metre, and the lattice is read at its nodes and across a face edge
- **THEN** the bytes agree for any thread count and differ with the seed, H and K stay in their ranges, the bands are 30 to 170 m thick, K steps by under 0.11 between samples a quarter metre apart, the stack's offset varies over the planet, the caprock flag follows the hardness, and the lattice reads the class source's terms within one code

### Requirement: GEN-122 The lithology preview
`generate --lithology-preview DIR [--seed N | --world F] [--km N]` SHALL write, with no GPU, equirectangular maps of the hardness, of K at the surface (caprock black) and 200 m below it, and of the soluble flag, a vertical cross-section through a mountain belt into a basin (K, caprock, the surface on top), and `lithology.txt` with the shares, the section's place and the band thickness against the share of the drop to the receiver that a pass-1 step removes. The text and the run's line SHALL say the images show the lithology field, an input, not terrain; the bytes SHALL not depend on the thread count.

Verify: A
Status: active
Source: ADR 0102 point 4, SPIKE-102 F1

#### Scenario: The preview
- **WHEN** the preview is made on 1 and 4 threads, written to a folder, and the option is combined with another generate command, a definition named twice, and `--km` alone or out of range
- **THEN** the images and the text agree for any thread count, the six files exist, the text carries the label, the shares and the band thickness, and each refusal names the option

### Requirement: GEN-123 The uplift field
An analytic uplift field SHALL give, at a direction, the uplift of a carve step in units of `k_dt` (the ratio `U / K` at K 1): CPU-canonical (`libm`, integer hashing), never stored, the same bits for any thread count, keyed by `UPLIFT_VERSION`. It SHALL be the largest of 24 (1 - craton) + 2 craton, 70 core, 32 back-arc and 28 rift (start values), times 1 + 0.2 noise (frequency 3), so that belts and arc coasts are highest, the back-arc medium and the cratons about 0. The terms SHALL come from the province lattice of GEN-116, which carries the class source's back-arc term as a seventh. A consumer SHALL fold `UPLIFT_VERSION` into its keys.

Verify: A
Status: active
Source: owner 2026-11-05, ADR 0103, ADR 0102, GEN-116

#### Scenario: The field
- **WHEN** the field is built on 1 and 8 threads and read at random points, at the province terms of a belt, a back-arc, a craton and a flat, and across a face edge
- **THEN** the bytes agree for any thread count and differ with the seed, the value is highest in a belt and lowest on a craton, the back-arc lies between, it is continuous across a face edge and finite and not negative everywhere

### Requirement: GEN-125 The uplift preview
`generate --uplift-preview DIR [--seed N | --world F] [--km N]` SHALL write, with no GPU, equirectangular maps of the uplift and of the uplift over the lithology's K at the surface, a section through a belt over a back-arc to a craton (the uplift and the base elevation), and `uplift.txt` with the land mean and quantiles, and the shares and mean U by term. It SHALL take the options and refusals of GEN-122. The text and the run's line SHALL say the images show the uplift field, an input, not terrain; the bytes SHALL not depend on the thread count.

Verify: A
Status: active
Source: owner 2026-11-05 (planner inbox #72), GEN-122, GEN-123

#### Scenario: The preview
- **WHEN** the preview is made on 1 and 4 threads, written to a folder, and the option is combined with another generate command, a definition named twice, and `--km` alone or out of range
- **THEN** the images and the text agree for any thread count, the four files exist, the text carries the label, the mean and the shares, and each refusal names the option

### Requirement: GEN-126 The channel block of a compact tile
A compact tile SHALL carry a channel block of 81 words after its driver block, one word per texel of the 9 x 9 grid: bits 0-7 the fill depth code of keep-pits (GEN-120), bits 8-15 the erosion depth code, every other bit zero; a code is `round(255 sqrt(depth / 64 m))` and the neutral word 0. A field that reads baked channels SHALL give both depths per texel and the builder write them; a texel without a depth or with a failed read SHALL be 0, and a slot read back with a reserved bit set SHALL be refused. `CHANNEL_BLOCK_VERSION` and the depth scale SHALL enter the channel identity of a field's climate version word (GEN-119).

Verify: A, C
Status: active
Source: planner inbox #67 (owner, 2026-11-05, ADR 0101, STRM-076), GEN-119, GEN-120

#### Scenario: The channel block
- **WHEN** a compact tile is built over a field that gives fill and erosion depths, over a field without channels, over fields wrapping one, with a failed read at a texel, and its slot is rebuilt from its bytes
- **THEN** each texel holds the square-root codes of its depths with every other bit zero, the field without channels has the zero block, every wrapper forwards the depths, a texel without a depth is 0, the slot round-trips and a set reserved bit is refused, and the keys with channels differ from those without

#### Scenario: The shader reads the block
- **WHEN** a tile holds a uniform channel block and the fill and erosion views are drawn, and a tile holds the neutral block
- **THEN** each view is the CPU mirror's false colour of the decoded depth within 0.003, and the neutral block shows 0 m

### Requirement: GEN-117 The orographic climate
A relief-aware climate SHALL give, at a direction, a wind vector for two seasons, the annual precipitation (the two seasons' rain summed, read per point), the aridity `P / PET` (`PET = 300 + 45 T`, at least 150 mm) and a seasonality value in `[0, 1]`. It SHALL be built from the base elevation alone, never from a class weight, by a fixed number of order-independent iterations (f64 forcing, f32 lattices), the same bits for any thread count, never stored, and keyed by `OROGRAPHY_VERSION` and its lattice size. Precipitation SHALL be higher on a windward flank than on its lee. A class-source world's precipitation channel SHALL be this value clamped to 0 to 4,500 mm, with `GLOBAL_VERSION` bumped.

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

### Requirement: GEN-128 Arid class rules
The default class source SHALL give the badlands class a weight where the rock is soft, the climate semi-arid, the height moderate and the relief broken, and the sand-sea class a weight where the climate is arid, the height low and the sand supply high, from the aridity of GEN-117 and the hardness of GEN-116, by smooth ramps with every threshold a named field of a parameter set whose default is the Earth preset. The weights SHALL be capped, blended over the base class weights at read time, and read by the base elevation and the carve factor; the climate SHALL NOT read them. Keys SHALL fold `ARID_RULES_VERSION` and the climate's identity.

Verify: A
Status: active
Source: owner 2026-11-05, ADR 0104 point 4, planner inbox #73, #74, GEN-116, GEN-117

#### Scenario: The rules
- **WHEN** the weights are read on soft semi-arid broken ground, on arid low supplied ground, where each condition fails alone, and across a sweep of the aridity and the height
- **THEN** badlands is the product of the soft, semi-arid, moderate-height and broken-relief ramps and has weight only where all four hold; sand sea is the product of the arid, low and sand-supply ramps and has weight only where all three hold; each is capped at 0.8 and both together at 0.85 (scaled down together beyond it); the blend rescales the other classes in proportion, replaces any arid share of the base, and sums to 1; a weight moves by under 0.08 and the blended `k` by under 0.25 per 0.01 of aridity or 10 m

#### Scenario: Parameters and keys
- **WHEN** a parameter set other than the Earth preset is given, and the rules' version, the climate's version or its lattice size change
- **THEN** the weights follow the parameters with no other literal in the kernels, and the layer identity of a bake changes with each of the three

#### Scenario: The preview
- **WHEN** `generate --arid-preview DIR [--seed N | --world F] [--km N]` is run, and combined with another generate command or a definition named twice
- **THEN** it writes the class map before and with the rules, the two weights and `arid.txt` (the land share of every class against Earth's, the step of the blended `k` before and after, the class weights at the terrain scorecard's arid sites), labelled as an input, not terrain, and each refusal names the option

### Requirement: GEN-118 Shelf profile
The default class source SHALL give the ocean floor a continental shelf: from the coast of the field the floor falls linearly to the shelf break over a width that blends smoothly between a passive and an active value by the margin activity at that coast, then down the continental slope to the abyssal depth. The distance to the coast and the margin activity SHALL be read from the coast lattice (GEN-136); the floor SHALL fade in from the coast and keep a minimum of water. Land SHALL be unchanged. Every depth, width and ramp SHALL be a validated field of `ShelfProfile` with the Earth preset below as default (ADR 0108); both versions change.

Verify: A
Status: active
Source: owner 2026-11-05 (planner inbox #83), SPIKE-102, ADR 0107, ADR 0108, planner inbox #84

The Earth preset of `ShelfProfile`:

| Parameter | Earth default | Kind |
|---|---|---|
| break depth, abyssal depth | 130 m, 4,500 m | Earth state |
| active width, slope width | 30 km, 90 km | Earth state |
| passive width | 80 km | calibration (Earth's mean shelf width, about 80 km; tuned so that the share of the ocean under 200 m is of Earth's order on a coast longer than Earth's) |
| active within (distance), active convergence | 150 to 450 km, 0.05 to 0.35 | calibration |
| coast lattice cells, coast ramp (land depth) | 512, 0.01 | calibration |
| minimum water, its ramp | 5 m, 0.01 | calibration |

#### Scenario: Profile
- **WHEN** the profile is read at 0 km, at the shelf width, at the foot of the slope and beyond, and along a walk of quarter-km steps
- **THEN** the floor is 0 at the coast, minus the break depth at the shelf width, minus the abyssal depth at the foot of the slope and beyond, never rises seaward, and has no step over 20 m in a quarter km

#### Scenario: Margin type
- **WHEN** the active weight is 0, 1 and between, far from any margin, and at a non-converging margin
- **THEN** the width is the passive value at 0 and the active value at 1 with a smooth blend and no switch, and a coast far from a margin or at a non-converging one is passive

#### Scenario: The field
- **WHEN** the field is sampled over the sphere
- **THEN** land is bit-identical to the field before the shelf (pinned points), the sea is between the abyssal depth and 0, the share of the ocean shallower than 200 m is of Earth's order, the abyss is reached, and at every coast of the field the floor meets the land within 5 m

#### Scenario: Parameters
- **WHEN** a parameter set has a non-positive or non-finite depth or width, a break not above the abyssal plain, a ramp with equal or reversed edges, or cells that are not a power of two
- **THEN** construction refuses it

#### Scenario: The preview
- **WHEN** `generate --shelf-preview DIR [--seed N | --world F] [--km N]` is run, and combined with another generate command or with `--world` and `--seed`
- **THEN** it writes the bathymetry map, a passive and an active coast region at 2 km and 10 km and `shelf.txt` (the shelf share of the ocean, and by margin type the distance from the coast of the field to the shelf break p10, p50 and p90 and to the foot of the slope p50, against Earth's), and refuses the combinations, naming them

### Requirement: GEN-134 Ice history
The glacial-maximum equilibrium line altitude SHALL be a field over the orographic climate: the sea-level temperature of the climate minus the cooling of the glacial maximum, less the temperature an equilibrium line needs at the rain there, over the climate's lapse rate. The needed temperature SHALL rise log-linearly with the rain, held to a range. Ground above the line SHALL carry ice by a smooth cover from the line to a ramp above it, and the sea none; where the line lies under the ground everywhere, the land is a continental sheet. Every value SHALL be a validated field of `IceParams` with Earth's values as default (ADR 0108); the field SHALL be deterministic and never stored.

Verify: A
Status: active
Source: owner 2026-11-05, ADR 0106 point 1, ADR 0108, planner inbox #76, #86, GEN-117

The Earth preset of `IceParams`:

| Parameter | Earth default | Kind |
|---|---|---|
| cooling | 6 C | Earth state |
| dry needed temperature, its rain | -10 C, 300 mm | Earth state |
| rise per decade of rain, range | 10 C, -12 to 2 C | calibration |
| cover ramp, land ramp | 200 m, 50 m | calibration |

#### Scenario: The line
- **WHEN** the needed temperature and the line are read over rain, sea-level temperature and lapse rate
- **THEN** the needed temperature is the dry value at the dry rain, near 0 C at 3,000 mm, monotone in the rain and held to its range; the line stands at the tropical sea-level temperature less the cooling over the lapse rate, lies under sea level where the sea level is cold, and falls with a colder sea level and with more rain

#### Scenario: The cover
- **WHEN** the cover is read below, at and above the line, at sea and over land under a line below sea level
- **THEN** it is 0 below the line and at sea, 1 from the ramp above the line over land, monotone and continuous between

#### Scenario: The field
- **WHEN** the field is read over a climate at 1 and 3 threads
- **THEN** the bits are the same, the cover is in `[0, 1]`, the line falls towards the poles, and the land under ice is of the glacial maximum's order

#### Scenario: Parameters
- **WHEN** a parameter is not finite, a rain or ramp is not positive, the cooling is negative or the range is reversed
- **THEN** construction refuses it

#### Scenario: The preview
- **WHEN** `generate --ice-preview DIR [--seed N | --world F] [--km N]` is run, and combined with another generate command or with `--world` and `--seed`
- **THEN** it writes the ice-extent map, the line map and `ice.txt` (the land share under ice against the glacial maximum of Earth, the line's quantiles, the mid-latitude highlands' share and the glaciated sites' cover), and refuses the combinations, naming them

### Requirement: GEN-136 Coast distance lattice
The class source SHALL read the distance to the coast of its own field from a lattice over the cube faces: per point the signed distance to the nearest sea-level crossing of the continent value (sea positive, land negative, at most a cap of the widest shelf plus the slope) and the margin activity at that crossing. The crossings SHALL be found on the lattice's cell edges, the nearest by a bounded search; the read SHALL be bilinear and continuous across face edges. The lattice SHALL be built on the first read and its bytes SHALL not depend on the thread count. The distance SHALL not read the field's gradient.

Verify: A
Status: active
Source: planner inbox #84, #89, GEN-118, ADR 0107, ADR 0108

#### Scenario: Exactness
- **WHEN** the field is a circular island and the lattice is read at points within the cap
- **THEN** the distance equals the arc to the island's edge within a quarter of a cell

#### Scenario: Sign, cap and no coast
- **WHEN** a point lies on land, in the sea, beyond the cap, or the world has no coast
- **THEN** the distance is negative, positive, the cap, or the cap with activity 0

#### Scenario: Continuity and threads
- **WHEN** the lattice is read at two directions 6 m apart across the sphere, and built on 1 and 5 threads
- **THEN** the read changes by under 0.01 and the bits are the same

#### Scenario: Parameters
- **WHEN** the cell count is not a power of two
- **THEN** construction refuses it

### Requirement: GEN-140 The band and crease noise can turn its lattice per octave
The noise of the hill band, the mountain band and the creases SHALL be switchable to a lattice turned by a fixed rotation per octave (axis and angle from `libm`, different for every octave), so no lattice plane of one octave is a plane of another or of the world axes.
- The switch SHALL be off by default and then every height, key and identity word SHALL be the bits of today; on, it SHALL change the identity words of the bake inputs and of the erosion field's climate word.
- The bounds of the turned noise SHALL be the bounds of the same cap in the turned frame and SHALL hold.

Verify: A
Status: active
Source: SPIKE-104 D-C, planner inbox #91 (owner, SPIKE-105 decision 1 = A conditional)

#### Scenario: Planes
- **WHEN** the octaves of the untouched noise and of the turned noise are summed and their slope is measured across the world planes `x = k / f`
- **THEN** the untouched sum is flat across them and the turned sum is not

#### Scenario: Switch off
- **WHEN** the switch is off
- **THEN** the identity words are unchanged and the turn is a rotation (lengths kept) that differs for every octave

### Requirement: GEN-157 The H0 re-route over an edit equals a fresh route of the edited heights
The edited heights SHALL be the base heights with the stamps added at the cell centres (`apply_stamps`), cells outside every item's reach keeping the base bits; the re-route SHALL be `GlobalGrid::route` on them with the sea as outlets, and SHALL equal bit for bit a route of the same heights made without the preview. The preview SHALL not depend on the thread count.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #95), A3 5.1 and 11.1, GEN-139

#### Scenario: Equality
- **WHEN** a height edit is previewed on a small planet and its heights are routed directly
- **THEN** the receivers, areas and outlets are equal, and cells outside the item's reach have the base's height bits

### Requirement: GEN-153 The memo caches of the edit path have a budget and eviction never changes a result
The memo caches of the edit path (`PartCache`, `RoundCache`) SHALL take an optional budget in bytes, a parameter with no machine count in the rule. Over it, after each round and at the end of a solve, a cache SHALL evict previous results (at the end only), tile floods and catchment rounds, each least recently used first (ties by key), and never data the running solve used; a count of what changed SHALL say when its previous result is gone. A solve after any eviction, an edit and its undo, and a replay SHALL equal the fresh build bit for bit (G-EQ). No budget SHALL evict nothing and `without_memory` SHALL keep nothing.

Verify: A
Status: active
Source: Owner 2026-11-09 (A3 6.6: cache eviction required), architect order inbox #99, GEN-143, GEN-146

#### Scenario: Eviction
- **WHEN** a four-round solve, an edit, its undo and replays run under budgets of a third of the unlimited size, of nothing and of one byte under it
- **THEN** every result equals the fresh build, the cache holds at most the budget after each solve, something was evicted, and an unlimited cache evicts nothing

### Requirement: GEN-161 The stage hooks only read
The stages of the new chain SHALL call the stage probe's `emit` at their boundaries with a closure that only borrows the stage's output: each round of the round solvers, and thermal, deposition and pit spreading of the sediment pass here; the landmass and H1 boundaries by the build that owns them, K7 after its own change. With no sink installed `emit` SHALL be one atomic load: the closure does not run and nothing is allocated. With a sink installed the hooks SHALL only read: the rounds and the sediment pass SHALL give the same output bits and reports with the probe on and off.

Verify: A
Status: active
Source: Architect order 2026-11-09 (inbox #100, owner decision: stage previews, briefing item 18), CON-14

#### Scenario: On and off
- **WHEN** the rounds and the sediment pass run on a compact landmass without a sink and with a sheet probe installed
- **THEN** every height bit, the round report and the sediment report are equal, and with the probe installed each stage has written its fields

### Requirement: GEN-163 The lake producer
The bake SHALL turn each kept basin of H1 into one lake record and SHALL produce the river segments without the channel cells under water, as a pure function of the hydrology and the surface in a fixed order (CON-14).
- The record holds the level of the steady balance (A3 section 5.5), class, overflow flag, `q_in`, `q_out` and area at the level; a dry basin has its floor as level and no water.
- The water of a lake is its basin's cells and the cells draining into them with `z` under the level.
- A segment under water ends before it and resumes at the first cell out, `Q_w` carrying `q_out` at an overflowing lake's spill.

Verify: A
Status: planned
Source: Architect 2026-11-10, item 13 finding 2, briefing item 15

#### Scenario: Bowl with a river
- **WHEN** a bowl with a known sill and a channel through it is produced
- **THEN** the lake is full to the sill, no channel cell of any piece lies in the bowl, and a piece or a whole segment starts at the spill cell

#### Scenario: Gentle shore and endorheic lake
- **WHEN** a lake has a shelf 0.2 m under its level that H1 did not label, or an endorheic lake stands far below its sill
- **THEN** the river is not drawn over the shelf, and in the endorheic basin it is drawn down to the water's edge

### Requirement: GEN-165 One river width law
The width of a river SHALL be `w = a * Q_w^b` metres for a routed discharge `Q_w` in m3/s, one law for the burn-in and for the drawing, with `a` and `b` bake parameters in the key (standard `a = 7.2`, `b = 0.5`). K7's burn-in SHALL cut a trench of `max(1 cell, w)`: every cell whose centre lies within `w / 2` of the line, lowered to the line's level and never raised; a channel without discharges or narrower than a cell SHALL burn as the one-cell line, bit for bit.
- The width distribution of the channel cells of a region (p50, p95, maximum, the share wider than one cell) SHALL be reported.

Verify: A
Status: planned
Source: Architect 2026-11-10, item 15

#### Scenario: Trench widths
- **WHEN** a straight channel is burnt with `Q_w` of 100 m3/s into 10 m cells
- **THEN** the cells within 3.6 cells of the line are at the bed level, none farther is touched, and a smaller discharge burns a subset

#### Scenario: The width cap
- **WHEN** the law gives a width over 32 cells (`DISC_RADIUS_MAX` 16, GEN-168)
- **THEN** the burn-in and the drawing both use 32 cells, never the uncapped law (architect, 2026-11-11)

### Requirement: GEN-167 The lake water mask of a tile
The bake SHALL decide for each cell of a tile of L which lake's water it is, on `B + L`, as a pure function of the tile's window, and store the runs in the record's water section.
- Core wet: the cell's H1 parent is a water cell of a lake and its height is under the lake's level.
- Ring wet: under the level, within 8 steps of a core wet cell, entering only cells whose H1 parent drains into the lake.
- A cell is in one lake only; labels index the tile's lake list (255 at most).
- The window reaches 8 cells beyond the core, so tiles agree at their edges in any order (CON-14).

Verify: A
Status: planned
Source: Architect 2026-11-10, design section 13

#### Scenario: Bowl, valley and hollow
- **WHEN** a lake with a sill, a valley below the sill and another hollow under the level are masked
- **THEN** only the lake's own water and its ring are wet; the valley, its sides and the hollow are dry

### Requirement: GEN-177 A region of a face is baked to a commit with a channel graph
A region bake SHALL take a rectangle of regional cells of one face, a base, a climate and a size spec, solve the regional surface, find the hydrology, and write the L tiles of the land tiles, the basin tables, the channel graph and the commit.
- A region within 4 regional cells of a face edge is refused.
- The bytes are the same at any thread count and on a second run.
- Every flood epsilon and the f32 choice come from the world's precision (GEN-175); a height beyond its radius R is refused.
- A world whose epsilon is not the profile's is refused before any work (the commit spec records none).
- No store format changes.

Verify: A
Status: active
Source: Architect 2026-11-12 (item 15, G-RT fixture), STRM-084, GEN-170

#### Scenario: A fixture region
- **WHEN** a region of a small fixture face is baked
- **THEN** the commit holds a graph, its channels read back through the reader and its tiles are present

### Requirement: GEN-168 The burnt channel matches the width law
`burn_channels_wide` SHALL lower every cell of a channel's 8-connected line to the channel's level there, SHALL raise no cell, and SHALL burn a trench whose mean width across the line (burnt cells per cell of arc length, the line's ends left out) is within 0.5 cell of max(1, w) on a line off the cell rows and columns, within 0.71 cell on a diagonal or curved line, and within 1.05 cells on a line on a cell row or column; at every column across an axis-parallel line at least floor(max(1, w)) cells SHALL be burnt.

Verify: A
Status: active
Source: architect, 2026-11-11

#### Scenario: Lattice limit
- **WHEN** a symmetric burn is centred on a cell row
- **THEN** its cell count is odd, so an integer w can differ by one cell from w; the 1.05 adds 0.05 for the line's ends
- **AND** the carve that follows widens the valley around the burnt channel; that is not a burn defect

### Requirement: GEN-169 The channel profile is monotone
The burn SHALL target, at each cell of a channel's line, the lowest of its level and of the heights found within L line steps upstream along the channel tree (L the tile's halo in cells), lowering only. A final pass at the end of K7 SHALL lower every land cell of each line to the lowest height within L steps upstream, lowering path cells only, booking the removed volume as eroded and exported. Channels SHALL be listed upstream first, ties by segment id. K7 SHALL refuse a tile with channels whose halo is under the smallest carry, and a bake or reader a channel graph whose margin is too small; the pass-2 kernel version SHALL be bumped.

Verify: A
Status: active
Source: architect, 2026-11-11

#### Scenario: A hole under the profile
- **WHEN** a cell of the line already stands under its level
- **THEN** the cells downstream are lowered to at least that height, and none stands above the cell before it

#### Scenario: Details of the burn and the final pass
- **WHEN** the burn and the final pass run
- **THEN** a tributary is a channel whose last cell is this channel's first cell, and a channel inside one cell is nobody's tributary; the heights are those found before any burn, the disc of a wide river burns to the same target, and the final pass reads the surface as it finds it
- **AND** the removed volume is booked as eroded and exported, so G-SED closes; the final pass runs after incision, thermal, deposition and pit spreading, and the pass-2 kernel version bump is carried by the tile keys
- **AND** the final pass reports the largest lowering, the cells lowered by more than 1 m and the cells where the cutoff binds
- **AND** a trench is at most 32 cells wide (`DISC_RADIUS_MAX` 16) and, with a width law, the burn carries L - 16 steps so that the disc of a line cell in the halo does not break partition independence; the halo holds both, and K7 refuses a tile whose halo is under 16 + 8 cells, naming both numbers (architect, 2026-11-11)
- **AND** the stored channel graph carries its margin (H1 cells beyond a block) in its spec, its key and each block, equal to the H1 cells that hold the tile halo plus one; a bake refuses a graph whose margin is smaller than its tile halo needs, and a reader refuses a block whose margin is smaller than its commit's halo needs, each naming both numbers
- **AND** channels are listed upstream first by `channels_from_hydro` and by the feed

### Requirement: GEN-172 A window cut gives the breach no floor
When the caller marks the land outlets on the border of the grid as window cuts (`CarveParams::window_cut`), the breach SHALL NOT take their height as a base: a receiver chain that ends in such an outlet SHALL have no base, while a chain that ends in an interior land outlet (a basin's rim) keeps the base of GEN-162. A region bounded by the sea SHALL have no window-cut outlet, and a build without the flag SHALL be bit-identical to the build before this requirement. The catchment key SHALL hold the outlet kind, only when it is a window cut, and the layer key SHALL differ with the flag.

Verify: A
Status: active
Source: Architect, 2026-11-12 (SPIKE-107)

#### Scenario: Valley behind a window cut
- **WHEN** a bowl whose border cells are land outlets runs a round with the border as a window cut
- **THEN** the breach cuts the cells beside the border far below the border cells' height, and with the border as a basin rim it does not
- **AND** for a masked grid the border is the border of its rectangle; the caller sets the kind of an outlet (the flag), it is not inferred from heights

### Requirement: GEN-173 Flats are graded by distance to the pour points
The canonical flood SHALL grade a flat by the Euclidean distance to its lower rim, not by the chessboard count: the level is `base + epsilon d / sqrt 2`, `d` the distance in cells to the nearest rim cell, every cell kept above the neighbour it drains to. A flat whose Chebyshev diameter exceeds `halo - 1` cells, or that touches the border of the grid, keeps the chessboard grade. The grade SHALL be a function of the flat and its rim only, so the resident flood and the partitioned route give the same bits on any tiling. No cell SHALL stand above its chessboard count.

Verify: A
Status: active
Source: Architect, 2026-11-12 (SPIKE-108)

#### Scenario: A square pit with one pour point
- **WHEN** a square pit drains through one outlet in its wall
- **THEN** the longest straight diagonal run of the receivers is under half that of the chessboard flood, and every cell has a lower neighbour

#### Scenario: Details of the grade
- **WHEN** a flat is graded
- **THEN** the production profile's halo is 128 cells, so the bound is 127; a flat of exactly the bound across a tile corner gives the same bits resident and partitioned
- **AND** a flat is a connected set of cells the fill lifted with one pour level, and its lower rim is the cells next to it at or below that level
- **AND** the non-canonical flood and cube grids are unchanged
- **AND** no cell above a lower outlet beside it stands above one epsilon over it (GEN-159 and GEN-162 keep their intent; their exact values follow this grade)

### Requirement: GEN-176 K7 river beds stay on or above the sea level and the channel cells are fixed after the burn
K7 SHALL burn every land cell of a channel line to `max(bed, sea_level)`, after the 0.5 m code of the bed has rounded down. The channel cells of a window SHALL be fixed for every stage after the burn: the incising carve, thermal, the deposition sweep and pit spreading treat them as boundaries, and the material that would have entered them leaves the tile as counted, exported sediment. Only the final channel pass (GEN-169) MAY change a channel cell after the burn, downward. The reports SHALL count every cutoff bound, the burn's own included. A tile with no channel SHALL be bit-identical to the build before this requirement.

Verify: A
Status: active
Source: Architect, 2026-11-12 (G-PROFILE on production geometry: R1h tile (21, 2), 72 rises)

#### Scenario: A low coastal plain
- **WHEN** a channel crosses a plain from +20 m down to a sea outlet and its beds run to -10 m
- **THEN** no land cell of the line stands below the sea level and no cell is above its upstream neighbour

#### Scenario: Details of the fixed cells and the reports
- **WHEN** K7 runs after the burn
- **THEN** the channel cells of a window are the cells of its lines and the cells the burn lowered; the cells of a line's sea outlet are burnt to their bed
- **AND** the final channel pass is the only stage after the burn that changes a channel cell
- **THEN** the incising carve's breach does not lower a fixed cell and its sweeps do not move it; a fixed cell neither gives nor takes in thermal, the deposition sweep and pit spreading
- **AND** the material is counted as `thermal_exported_m3` and `deposit_exported_to_fixed_m3`, with `fixed_cells`, in the result and the MARK line; thermal's is a report only (the givers' loss is already the deposition sweep's eroded column), the sweep's own export to fixed cells is part of its ledger, so G-SED closes; a pit spreads from its free neighbours only, keeping the volume
- **AND** the final pass's `cutoff_binds` counts every path cell of the window, sub-sea cells included (on the cells' own heights), and the burn reports its own `burn_cutoff_binds` (the cells of the window whose floored target differs from the unbounded carry), both in the MARK line and the G-PROFILE ledger row with the distribution of what a cutoff costs, the bounded target minus the unbounded one in metres (p50, p99, max)
- **AND** G-PROFILE gates that no line step rises over 0.25 m and that no cutoff changes a height by more than 0.25 m; a cutoff above it comes to the architect with its tile and cell

### Requirement: GEN-166 The round surface is stored f32
When `CarveParams::round_f32` is set, the heights SHALL be narrowed to f32 (`f64::from(z as f32)`: round to nearest, ties to even) once at the end of every round and after the last round, and SHALL be read widened by every reader. The round SHALL compute in f64 and the breach is not narrowed separately; a height that does not fit an f32 SHALL be refused. The narrowing SHALL be idempotent, and every route to the round result SHALL give the same bits under the flag. The flag SHALL be part of every key that holds a round result; with it off every result and key SHALL be the bits of the build before this requirement.

Verify: A
Status: active
Source: A3 section 9, architect 2026-11-10 (D24 row 8)

#### Scenario: Round solver and carve under the flag
- **WHEN** a region is solved by the whole-region carve and by the round solver on 1, 3 and 8 threads with the flag set
- **THEN** every height is f32-representable and the results are bit-identical

#### Scenario: Details
- **WHEN** the flag is read
- **THEN** the layer key carries it by a byte and the catchment key by a bit, only when set
- **AND** the memoised solve refuses a run without the canonical flood, naming the reason, and pass 2 and K7 refuse the flag
- **AND** the flood's epsilon exceeds the f32 spacing at the largest height of the region (9.77e-4 m up to 16,384 m), above which carved epsilon chains collapse to equal heights

### Requirement: GEN-178 The plan stores the round surface f32 until the sediment pass
Under `round_f32` the resident plan SHALL hold the heights after the rounds as f32 (exact: the surface is f32-representable) and drop the f64 copy until the sediment pass, which SHALL read them widened. The flood, the weighted route, H1, the river pieces, the channel graph and the plan's drivers SHALL read heights through a trait with an f32 and an f64 form; every result and key SHALL be the bits of the plan on f64 heights. With f64 rounds the plan SHALL keep f64 heights. The saving SHALL be at least 200 MiB of the peak commit at k = 4, or the step is dropped.

Verify: A
Status: active
Source: Architect 2026-11-10 (GEN-166 row, the 200 MiB bar), inbox #125

#### Scenario: Same bits
- **WHEN** the plan runs with `round_f32` on a small compact cube
- **THEN** its H1 and sediment results equal the steps run by hand on f64 heights, and the allocation marks show the heights stored f32 after the rounds and widened before the sediment pass

### Requirement: GEN-188 The erosion layer's runtime read is continuous across cell-centre lines and exact at the cell centres
The runtime read of the erosion layer (`ErosionLayer::change_at`) SHALL be a monotone C1 cubic per axis, rows first, then across them: the Hermite cubic through the two middle samples, with the slope `2ab/(a+b) * 2ab/(a²+b²)` where the neighbouring differences `a` and `b` share a sign, else 0. Away from the cube-edge blend the read SHALL stay within the four cells around the point without a clamp, step at most 1e-4 m across a cell-centre line, kink at most 5 m per cell inside a cell (scenario below), and equal the stored sample at a cell centre (`CENTRE_SNAP_CELLS`, 1e-9 cells). `LAYER_READ_VERSION` 2 enters the field's keys.

Verify: A
Status: active
Source: Lead 2026-11-13 (order #110, under P2 E1), the #108 diagnosis of F9 frame-13715 (the four-cell clamp stepped at cell-centre lines: 1.5 % of crossings over 0.1 m on R3a, up to 12.5 m), numerics review 2026-11-13 (a harmonic-mean slope kinked inside the cells), ADR 0122 (detail pinned to the stored samples)

#### Scenario: No step at a cell-centre line
- **WHEN** a layer with cell-scale noise of 25 m is read on both sides of a cell-centre row or column, 1e-7 cells apart, at 20,000 random crossings away from the cube edges
- **THEN** no pair differs by more than 1e-4 m

#### Scenario: No crease inside a cell
- **WHEN** the same layer is read along a 6-cell scan at 1e-4 cells, along x at a fixed fraction of a row and along y at a fixed fraction of a column, 60 scans
- **THEN** no second difference over the step exceeds 5 m per cell (the harmonic-mean slope alone gives about 15-20)

#### Scenario: The sample at a cell centre
- **WHEN** the layer is read at the direction made from a cell's indices, on a 256-cell face and on the production face of 98,304 cells
- **THEN** the read is the stored sample (f32 widened) bit for bit

#### Scenario: No overshoot
- **WHEN** the layer is read at random points away from the cube edges
- **THEN** every read lies within the range of the four cells around the point

### Requirement: GEN-187 Where the layer is read the fine detail is pinned to the stored samples
Where the erosion layer is read (GEN-112) the field's fine-detail term (ADR 0098) SHALL be the detail minus its bilinear interpolant from the four layer cell centres around the point, 0 at a centre, so the drawn ground equals the stored base plus the faded change at every cell centre, bit for bit. Within two cells of a cube edge the interpolant is cross-faded with the neighbouring face's, so the term has no step there. The height bounds SHALL hold the pinned detail, a batch SHALL read the centres as single reads do, and the pinning's version SHALL enter the keys of every field that reads the layer.

Verify: A
Status: active
Source: ADR 0122 point 1 (accepted, owner 2026-11-13), SPIKE-113, VIEW-072's probe (the drawn ground minus B + L on R3a: p50 -2.1 m, all of it the detail)

#### Scenario: A cell centre
- **WHEN** the field with the layer is read at the centre of a layer cell more than two cells from a cube edge
- **THEN** the height is the base elevation plus the coast-faded change there, bit for bit

#### Scenario: A cube edge
- **WHEN** the field with a layer of zeros is read at pairs of points 0.13 mm apart across every cube edge and corner
- **THEN** no pair differs by a millimetre

#### Scenario: A batch over a blocked source
- **WHEN** sparse, dense, edge and cross-face batches are read over a source with blocks
- **THEN** every height is finite and equals the single read, and no error is recorded

#### Scenario: Between the centres
- **WHEN** it is read between centres
- **THEN** the detail is the unpinned detail minus its interpolant from the four centres, a batch reads as single reads, and the height lies within the bounds of every cap that holds the point

### Requirement: GEN-120 The bake keeps its pits and records how deep each lies below its spill level
The bake's carves SHALL keep the pits of the carved surface in the layer's change and SHALL record each pit's depth, so that the runtime shows the filled look until lake water and a pit-tolerance rule exist:

Verify: A
Status: active
Source: Owner 2026-11-05 (SPIKE-102 D1: keep every pit that survives the breach, store the fill depth, the runtime adds it back); SPIKE-102 section 4.1 row 2 and section 6; planner inbox #67 (the post-pass); GEN-100, GEN-101, GEN-121

Detail: - pass 1 (GEN-100): the route runs over the filled copy (the flood's receivers and area), but a cell that stands below its receiver is not raised to it; it gains its uplift only, so the donors above it carve toward its floor and the basin does not eat the carve's relief (SPIKE-102 run (b): the high mountains' 3 km relief p50 0.85 of Everest's against 0.62, the mountains' 0.94 of the Alps' against 0.86, and 0.85 and 0.94 on the filled surface too). The option is `keep_pits` (on); an incise-only carve (pass 2) never raises, so it ignores it;
- pass 2 keeps its pits (its local carve only incises) and leaves the tile's final heights as bake scratch (STRM-077); it computes no depth of its own: a window's flood cannot know a basin wider than its halo (the keep-pits preview: 152 m of disagreement across a tile seam);
- the depth of a core cell is how far the surface the runtime shows (the exact base plus the stored change) lies below the planet-wide spill level of the pass-2 surface (GEN-121), clamped at 0, `round(depth * FILL_STEPS_PER_M)` in a 16-bit code; only the cells that are pits of the pass-2 surface (the level above their final height) are given a depth, and a cell whose exact surface stands at or above the level (an outcrop inside the basin) keeps its height with depth 0; so the filled floor of a basin is flat against the bake's exact base (`BakeInputs::base`) to the code's step, the interpolation ripple of the pass-2 base being taken out; the runtime shows the stored bake's lattice read plus the crease detail and fades the change near the coast, so a floor at runtime differs from the bake's by the lattice read's error (docs/learnings.md 2026-11-04: 3.4 m rms on mountains) and the detail, which the depth does not touch (4 steps a metre, 0.25 m, owner 2026-11-05; the step is a parameter of the channel's key); a cell at or below the sea level, and a cell on a flat at its spill level, hold 0;
- the planes SHALL be the same bytes on any thread count (CON-14);
- a reader that shows the filled look SHALL add the depth back to the stored change (`TileResult::with_fill_added`); a tile without a fill plane is read as it is.

#### Scenario: A kept pit
- **WHEN** a plateau with a 20 m bowl and a channel to its edge at 60 m is carved with `keep_pits` and without, and the fill depth of the kept surface is taken
- **THEN** the kept pit stays at or under its 20 m while the filling carve raises it to over 59 m, the depth of the pit is its spill level minus its floor to 1 m, no outlet holds a depth, no depth is under 0.5 m, the same input gives the same bits, and an incise-only carve is the same bits with either setting

#### Scenario: Pass 2 keeps heights, not a depth
- **WHEN** one synthetic tile is run on 1 and on 8 threads
- **THEN** the final heights are the same bits on both, finite, within the base's range of the base plus the change, and the tile holds no depth plane

#### Scenario: The depth below the planet-wide level
- **WHEN** the final heights of the 16 tiles of a face with basins across tile edges and across a four-tile corner are post-passed, and compared with a global flood of the assembled face
- **THEN** every cell's code is the oracle's, and the runtime layer reads each cell as the stored change plus the depth

#### Scenario: A flat floor against the exact base
- **WHEN** a layer is baked from a smooth bowl terrain with the exact base given to the post-pass, and the basins of 20 cells or more are read as the exact base plus the stored change plus the depth
- **THEN** the cells of one connected basin hold one surface to within the code's step (0.25 m) of their median

### Requirement: GEN-121 The fill level is the planet-wide spill level of the final surface
The fill depth of GEN-120 SHALL be a function of the whole surface and not of a tile's window: the fill level of a cell is its planet-wide spill level `L(c)`, the lowest value of the highest height along a path from the cell to an outlet (the sea), and the depth is `L(c) - z(c)`. The level SHALL be computed in the form of Barnes et al.'s parallel priority-flood (2016), minimax, with no store and no base:

Verify: A
Status: active
Source: Owner and planner 2026-11-05 (inbox #67: the keep-pits preview's seam failure; option (a), the post-pass); SPIKE-102 section 6; GEN-120; Barnes, Lehman and Mulla 2016 (parallel priority-flood)

Detail: - stage 1, per tile: a priority flood from the tile's outlets and from every non-outlet cell of its border (each a seed with its own label, the index along the perimeter), in order of height with ties by the cell index; every cell takes the label of the seed that reaches it first and `level`, the highest height on its path from that seed; the tile returns the territories' adjacency, `(a, b, w)` with `w` the lowest `max(level, level)` over the neighbouring cell pairs of two territories (8 neighbours), sorted;
- stage 2: a graph whose nodes are the sea and the seeds of every tile; edges are the tiles' territory edges and the pairs of neighbouring border cells of two tiles with `w = max(z, z)`; the caller names the pairs, so a cube edge, a corner and a rotated gluing are pair lists like a planar seam; the planet-wide level `G` of a seed is the lowest highest-edge over the paths from the sea (Dijkstra with `max` for the sum; `+inf` where no path reaches);
- stage 3: the level of a cell is `max(level(c), G(label(c)))`, and `level(c)` for a cell reached from an outlet of its own tile;
- the result SHALL equal the minimax level of the whole surface bit for bit (the tests' oracle is a global flood over the same neighbour lists), and the same bits on every run (the kernels are single-threaded with a fixed order and no thread-count input, CON-14); heights that are not finite SHALL be refused.
The level does not change GEN-100's routing (a face does not route across its cube edges): the fill level is not a route, and the pairs the caller names are where basins join. GEN-121 replaced GEN-120's pass-2 tile rule (the depth below the window's flood); the depth of GEN-120 is taken against the exact base and clamped at 0, and the kernel's `L(c) - z(c)` is against the heights it is given. The code's cap and its step stay with the caller (STRM-077); the level has no epsilon and no minimum depth: a dent of any size is a pit.
The kernel holds no store, no tile geometry and no quantisation: The post-pass (STRM-077) feeds it the tiles' final heights (f32, exact).

#### Scenario: The tiled level equals the global flood
- **WHEN** 12 random terrains of 3 x 3 tiles of 10 cells with basins across tile edges and tile corners, a bowl of a flat 60 m floor across three tiles with a notch at 70 m to the sea, two tiles glued with a rotation (the east column of one against the top row of the other, 8-connected) with a bowl centred on the glued corner, nested basins on a plateau of equal heights, a tile of sea and a world with no outlet are levelled tile by tile and by one global flood
- **THEN** every cell's level is the same bits in both, the wide bowl's three tiles all read 70 m, a world with no outlet reads `+inf` at every cell, the same input gives the same bits and a height that is not finite is refused; a border cell that is an outlet is the sea through `seed_of`, an edge weight that is not finite is refused, and -0.0 and 0.0 are one height

#### Scenario: The perimeter
- **WHEN** the perimeter index of every border cell of tiles of 3 x 3 to 10 x 12 cells is taken and inverted
- **THEN** each border cell has one index, an inner cell has none, and the inverse returns the cell

### Requirement: GEN-129 The carves read the runoff as discharge and the vegetation as a local erodibility factor
Both carves SHALL take an optional runoff weight per cell and a vegetation factor in their erodibility.
- The runoff of a place SHALL be `R = P - E` in mm a year, `E = P PET / (P^n + PET^n)^(1/n)`, `n` 2.6 (`BUDYKO_N`), `P` the annual precipitation and `PET` the potential evapotranspiration of the orographic climate (GEN-117); `R` SHALL be 0 for `P = 0`, increase with `P` and never exceed `P`.
- The weight of a cell SHALL be `w = clamp(R / RUNOFF_REF_MM, WEIGHT_MIN, WEIGHT_MAX)` with `RUNOFF_REF_MM` 300, `WEIGHT_MIN` 0.02, `WEIGHT_MAX` 8.

Verify: A
Status: active
Source: ADR 0104 point 3 (amended), ADR 0108, GEN-117, GEN-127, GEN-110

Detail: - A routing given weights SHALL start each cell with the area `w dx^2` and share it by the multiple flow direction as before: the area it returns is the discharge `Q = sum(w dA)`, equal to the area for weights of 1; the total at the outlets SHALL be the sum of the weighted areas.
- The carve's stream-power number SHALL be `k_dt K sqrt(Q) / dx`; a cell below a dry reach SHALL keep the discharge of the wet catchment above it. The area cap of pass 2 applies to `Q`. This amends GEN-110: the stored area channel of a bake with weights holds `round(8 log2(Q / cell area))`, an effective drainage area in cells of reference runoff; a cell whose `Q` is under one cell has code 0 (a desert cell reads as one cell) and a wet cell reads up to `log2(8)` = 3 steps of 8 (24 codes) above its area. Readers of the channel (the wetness driver of GEN-119, `drivers::AREA_LOG2_DRY` and `AREA_LOG2_WET`) read discharge; the channel is keyed through the `rock` byte, so no record of a bake without weights is read as one with them.
- The vegetation factor SHALL be `V_FLOOR + (V_PEAK - V_FLOOR) u e^(1-u)`, `u = P / VEG_PEAK_MM`, with `VEG_PEAK_MM` 300, `V_FLOOR` 0.5, `V_PEAK` 1.5: `V_PEAK` at `u = 1`, `V_FLOOR` at `P = 0` and for `P` much larger; `BakeInputs::climate_k` SHALL multiply the lithology's K by it.
- The `rock` key byte SHALL carry bit 2 (weights) and bit 3 (vegetation); `BakeInputs::id` SHALL fold `RUNOFF_VERSION` and a digest of the constants (`runoff::constants_identity`, pinned by a test). With no weights and no vegetation the carves SHALL be those of GEN-127 bit for bit.
- Every Earth-specific value of the runoff and the vegetation SHALL be a named constant of `generators::runoff` with Earth as the default (ADR 0108); the PET law is the orographic climate's (GEN-117), which `pet_mm` reads.

#### Scenario: The runoff in the carve
- **WHEN** the runoff, the weight and the vegetation factor are evaluated over a grid of `P` and `T`, a routing is run with weights of 1, with a wet catchment above a dry reach and with random weights, a cone is carved with wet and dry weights, with weights of 1 and with none, a face is carved with a halo and a tile with weights read at the cells' directions, the climate erodibility and weight of a bake are read on 1 and 8 threads, and a store written with other bits is run
- **THEN** the runoff has the limits above, the weighted area equals the unweighted for weights of 1 bit for bit and its outlet total is the weighted sum, the discharge below the dry reach is at least the wet catchment's, the dry ramp incises less than the wet one, the bytes agree for any thread count, and a mismatch of the key bits is refused

### Requirement: GEN-145 H1 derives the channel graph and the basins from a routing
H1 SHALL derive from a routing (clipped MFD area, GEN-141) the channel graph and the basins, kept apart from the diffuse field.
- A cell with `A_d >= T` (the runoff-weighted diffuse area, `T` a parameter, default 75 km^2) SHALL start a channel; every D8 receiver of a channel cell SHALL be a channel cell or an outlet, and its filled level SHALL be lower (a channel continues downstream and does not restart).

Verify: A
Status: active
Source: A3 sections 5.2 to 5.5 and 12.1 (G-CON), inbox #99, ADR 0111 (proposed)

Detail: - The channel discharge `Q(c)` (the potential discharge, before the losses of basins; a basin's `q_out` is the discharge that continues below it) SHALL be the local term `a(c) r(c)` plus `Q` of every D8 donor (zero local term in an outlet cell); the sum of `Q` at the outlets SHALL equal the sum of the local terms over the land to a relative error of 1e-9, and `A_d` at the outlets SHALL equal the sum of the (weighted) true cell areas of the cells that drain to them to 1e-9, also across cube edges.
- A segment SHALL run from a head or a confluence to the next confluence or the mouth, its id the hash of the body, its downstream end cell and its upstream end cell; every channel cell except the mouths SHALL be in exactly one segment (counted per cell), and every segment SHALL reach a mouth through its downstream segments.
- A basin SHALL be a connected set of cells whose filled surface stands more than `pit_min_m` above the ground, with its floor, its spill point (the rim cell of the lowest filled level), a 16-step area and volume curve (parameters: `pit_min_m` 0.5 m; artefact below 4 cells and shallower than 2 m), its class (lake with an active outlet, endorheic without) and water level by the steady balance, and an id that is a function of its lowest cell (ties by the global cell id) only. A basin of fewer than `min_cells` cells (default 4) and shallower than `min_depth_m` SHALL be an artefact, counted and not kept; a component shallower than `pit_min_m` from floor to rim SHALL be a ramp of the flood's epsilon, counted and not kept; the inflow SHALL count each cell that drains into the basin once (superseded by the cascade of GEN-146, which also removes the flow an upstream basin lost); a basin that holds a flagged cell SHALL be of class intentional (GEN-146).
- The run on the prototype landmass (`examples/hydro_landmass`) is a measurement, not a gate; the gates are the tier-A tests.

#### Scenario: channels and basins
- **WHEN** lands of ridges and of terraces (with and without a runoff field), and a sphere of 24 cells a face edge with water crossing cube edges, are routed with the round rules and given to H1; and a land with a bowl is given strong and no net evaporation and edited
- **THEN** no segment is disconnected, no channel cell lacks a downhill channel receiver or an outlet, `Q` is conserved at confluences and at the outlets, a channel and a segment cross a cube edge, the bowl is one lake or one endorheic basin with its floor, spill level, curve and water level, its id moves only when its lowest cell does, and a one-cell dent is an artefact

### Requirement: GEN-138 The grids of the bake may carry the physical cell metric
A `Grid` over a window of a cube face SHALL be able to carry a physical metric: the area of each cell and the distance between neighbouring cell centres of the equiangular face (`TangentWarp`) in place of the uniform `dx^2`, `dx` and `sqrt 2 dx`.

Verify: A
Status: active
Source: SPIKE-105 §12 F5, inbox #97 (owner D1, 2026-11-08)

Detail: - The area of a cell SHALL be the exact spherical quad: the solid angle of the gnomonic patch between its edge tangents times the radius squared; the cells of a face SHALL sum to a sixth of the sphere (relative error under 1e-12 at 257 cells an edge) and the cells along opposite edges of a face SHALL be equal (the equiangular map is symmetric, so faces that meet at a cube edge agree on their edge cells).
- The distance SHALL be the great circle between the centre directions; at the face's centre it SHALL be `dx` on the axes and `sqrt 2 dx` on the diagonals, at a corner about 0.94 `dx` on the axes, and `d(a, b) = d(b, a)`.
- The routing SHALL take the receiver slope, the multiple-flow-direction slope and the starting area of a cell from the metric, so that the area returned at the outlets is the sum of the cell areas; the carve SHALL take the stream-power distance and the breach slope from it.
- `CarveParams.cell_metric` SHALL be false by default, and a grid without a metric SHALL give the uniform values bit for bit; a layer baked with the switch on SHALL carry it in the layer key and one baked with it off SHALL keep its key.

#### Scenario: The physical cell metric
- **WHEN** the cells of faces of 8, 64 and 257 cells are summed, the edge cells of opposite sides compared, the distances read at the centre and at a corner, a routing is run over a face with a halo and a carve twice with the metric and once without
- **THEN** the sums are a sixth of the sphere, the edge cells are equal, the distances are those above and symmetric, the outlets hold the total area, the two metric runs are the same bits and differ from the uniform run, and a grid without a metric gives `dx^2`, `dx` and `sqrt 2 dx` exactly

### Requirement: GEN-127 The carves read the erodibility and the uplift fields
Both carves SHALL take an optional rock: the erodibility `K(cell, z)` of GEN-116 with the lower edge of the layer the height lies in and, for pass 1, the uplift `U(cell)` of GEN-123. This amends GEN-100 and GEN-101: the class carve factor multiplies `K dt` (erodibility), and a given uplift is not scaled by it.
- In each step, before the sweep, the erodibility of every cell SHALL be evaluated at the cell's height at the start of the step (in parallel over rows, a value per cell: the same bits for any thread count) and SHALL multiply the class carve factor in that step's implicit update; it SHALL be finite and not negative.

Verify: A
Status: active
Source: ADR 0102, ADR 0103, GEN-100, GEN-101, GEN-116, GEN-123

Detail: - A step SHALL NOT lower a cell below the lower edge of its layer by more than `LAYER_SKIP_M` (0.01 m); the room above the edge is kept as a 16-bit count of 4 mm (rounded up, so a cell may end up to 4 mm further below), a room of 262 m or more is no edge; the rest of the lowering is carried to the next step, which reads the layer below. An edge SHALL be a number; an edge above the height SHALL NOT raise a cell.
- A given uplift SHALL replace `u_over_k`: a cell gains `k_dt * U * taper` in a step, the taper the smoothstep of the cell's starting height above the sea over `uplift_taper_m` as before; the uplift SHALL NOT be scaled by K or by the class carve factor (ADR 0103 point 3), and SHALL be finite and not negative. An erodibility without an uplift array SHALL NOT scale the uniform uplift.
- Pass 2's local carve (incise only, no uplift) SHALL read K and the layer edge.
- With no rock, or with `K = 1` and `U = u_over_k` (a value `f32` holds exactly, as 20) and a carve factor of 1 or none, the carve SHALL be bit for bit the carve without fields.
- `BakeInputs::rock_k` SHALL multiply the lithology's K by `ROCK_K_SCALE` = 1.16, a fixed constant of the default lithology (a softer or harder world erodes more or less): the `c` for which the land mean of `g(c K f)` equals that of `g(f)`, `g(f) = f / (1 + f)`, `f = k_dt sqrt(A) / dx` with `A` the drainage area in cells, `P(A > a) = a^-0.43` for 1 to 1e7 cells, K the raw erodibility at the surface of land; the geometric mean over 12 seeds of the per-seed `c` (0.88 to 1.78; `k_calib`). The carve then removes the mean incision of a unit rock on the same drainage in this model; relief and elevation still differ (design notes).
- The layer and tile keys SHALL carry the fields present (a `rock` byte: bit 0 the erodibility, bit 1 the uplift) and a run whose fields differ from the spec's SHALL be refused; `BakeInputs::id` SHALL fold `LITHOLOGY_VERSION`, `UPLIFT_VERSION` and the bits of `ROCK_K_SCALE` into the keys of every bake (a bake without fields is keyed like one with them).

#### Scenario: The fields in the carve
- **WHEN** a cone is carved with K = 1 and U = `u_over_k` (no factor and a factor of 1), with K 0.12, 1 and 1.8, with K 0 and the factor 1 and 5, with K and no uplift array, with a K that records its reads on 32 and on 130 rows, with a caprock over soft rock on a steep ramp with and without the edge over 1 and 3 steps, with an edge above the height and a NaN edge, on 1 and 8 threads, with a negative K and a negative U, and a face is carved with direction fields on 1 and 8 threads; the factor solver is run on a constant K, the lithology's lower edges are probed, and the id is built with other versions and another scale
- **THEN** the first equals the carve without fields bit for bit, a harder rock settles higher, a K of 0 lifts each cell by exactly `steps k_dt U taper` whatever the factor, K alone leaves the uniform uplift, step 2 reads the heights that step 1 left, every cell is read once at its own height, a caprock forms a step and holds the cell in the following steps, no cell is raised by an edge above its height, the bytes agree for any thread count, the NaN edge and the negative inputs are refused, the face's change differs from the one without fields, the solver reproduces the incision of a unit rock and the constant lies among the seeds' factors, a band's lower edge is where the band below begins, and the id differs with each version and with the scale

### Requirement: GEN-162 A breach does not cut below its chain exit
Under the local clip of the round rules (the round solver's rules) a breach SHALL NOT lower a cell below the base level of its chain exit: for a receiver chain that ends in an outlet above the sea level, the base of every cell of the chain is the outlet's height, and the target of a breach is at least that base. A chain that ends at or below the sea level SHALL have no base, and every other rule set (the default rules, K7's and the sediment pass's canonical flood without the clip) SHALL keep today's breach.

Verify: A
Status: active
Source: Architect, 2026-11-10

Detail: - A depression whose exit is fixed (A3 sections 5.3 and 6) SHALL NOT get deeper from round to round through the breach.
- A build whose outlets are all at or below the sea level SHALL be bit-identical to the build without the rule.

#### Scenario: Lake behind a cut outlet
- **WHEN** a closed bowl whose rim is its only outlet runs four rounds
- **THEN** its median depth under the fill grows by at most 10 % of the starting depth in round 1 (the incision of the sweeps; a bowl without the breach grows by 4.6 %) and by at most 0.25 m in any later round

#### Scenario: Outlet at the sea
- **WHEN** a bowl whose rim is at the sea level runs one round
- **THEN** no cell is more than 1 m under the fill


### Requirement: GEN-103 The default class source and the whole-planet preview
`planet-generators` SHALL provide the default class source of ADR 0089 decision 3 as CPU fields behind the class-weights interface of decision 1: the plate mask (GEN-094), the memberships (GEN-095) and the base elevation without the hill band (GEN-096) as pure functions of the seed and the direction, the class weights as `u8` on the lattice of GEN-093 (bilinear reads, continuous across face edges), the untapered blended `k` (GEN-097) and the hill band's amplitude from `k` (GEN-098). They SHALL use `libm` and integer hashing only and give the same bits for any thread count (CON-14). The source SHALL NOT change any stored field or `GLOBAL_VERSION`; wiring it into the bake is `m9-bake`'s.

Verify: A
Status: active
Source: opus-planner (2026-11-01, inbox #37), ADR 0089 decisions 1, 3 and 10, GEN-093 to GEN-098; SPIKE-77, SPIKE-79, SPIKE-80, SPIKE-91

Detail: `cargo xtask preview world --seed N [--out DIR]` SHALL write, from those functions and no store, equirectangular PNGs of at least 2048 x 1024 under `target/preview/world-seed-N` or `--out`, in under a minute on the reference machine:
- land and sea; the elevation with hillshade; today's elevation with hillshade; both side by side;
- the plate mask with the plate margins; the plateau field and the back-arc;
- the relief class by largest weight and by the blended weights; `k`.
Every image SHALL carry the seed, `GLOBAL_VERSION`, `MESH_VERSION`, the class list and class source versions, the preview version, and a legend; the command SHALL also write a report of the class shares, the share of land above 2 and 4 km, the highest land, the largest step of `k` between lattice points and the share of steps over 2.0, and the shape metrics S1 to S7 of SPIKE-91 beside their Earth references (computed on the preview grid; S8 is not measured).

#### Scenario: Same seed, same bytes
- **WHEN** the lattice and the preview images of seed 1 are built on 1 and on 8 threads, and again for seed 2
- **THEN** every lattice point's weights sum to 255, the bytes of the lattice and of every image are equal between the thread counts, and seed 2 differs from seed 1

#### Scenario: Memberships and the blended k
- **WHEN** a point has core 1 and base elevation 3500 m, another has no mask term and plateau field 1, and a third has weights 128 hills and 127 mountains
- **THEN** the first holds weight 255 in high mountains, the second 255 in plateau, and the third's untapered `k` is `(128 x 5 + 127 x 6) / 255`

### Requirement: GEN-104 The continent stage
The global stage's continentalness SHALL be the continent stage of `planet-generators`, a pure function of the seed and the unit direction, `GLOBAL_VERSION` 6. Its raw value is the sum of:
- the shape: an eight-octave fBm of base frequency 1.6 and gain 0.72 per octave (each octave its own layer), read at the direction plus 0.6 times a three-octave warp of frequency 1.3, times 1.7, times `0.65 + 0.35 * 4 w (1 - w)`;

Verify: A
Status: active
Source: SPIKE-92 round 3 (H2r3), planner (2026-11-02, inbox #41), ADR 0089 decision 10

Detail: - the craton bias `0.8 * sum` of the lobe weights `smoothstep(0, 0.25, z) exp(-(u^2 + v^2))`, `z` the cosine of the angle to the lobe's centre (the weight is 0 behind it, and fades over `z < 0.25` so that the bias is continuous across the great circle 90 degrees from the centre), `u` and `v` the tangent angles `atan2` of the direction's components along the lobe's major and minor axes and `z`, over its radii, read at the direction plus 0.3 times the warp, with `w = min(1, sum)`;
- the islands (GEN-106).
A craton is a chain of lobes; the cratons hold the land shares 0.50, 0.26, 0.10, 0.06, 0.03, 0.02, 0.01, 0.01 of 29 % of the sphere (a cap of half-angle `r` holds `2 pi (1 - cos r)`), the two largest (0.50 and 0.26, at least 20 % of the land) as 3 lobes, a craton of 4 to 20 % as 1 to 3 (drawn), the others one; a lobe has aspect `a` uniform in 1.5 to 2.5 (radii `r sqrt(a)` and `r / sqrt(a)`), lobe centres 0.6 times the sum of the two major radii apart on a spine that turns by up to 1.2 rad per lobe, and craton centres at least the two extents plus 0.1 rad apart (up to 200 draws). The per-seed offset is the `1 - 0.29` quantile of the raw value over 20,000 directions of a Fibonacci sphere; the continentalness is the raw value minus the offset plus the sea level. The octave series is normalised by its full weight sum, so that a value with fewer octaves is the full value without its detail terms. Two such values SHALL be exposed: the smooth value (the first 4 octaves, no islands) for the distances of the class source (GEN-094), and the coarse value (the first 3 octaves, no islands) for the landform driver. The landform driver and the hill band's strength read the coarse value (GEN-064, whose text this change edits in `openspec/specs`, synced here because `m9-bake` removes GEN-064 and its delta cannot also modify it). `GLOBAL_VERSION` is 6 (`m9-bake`'s GEN-096 makes it 7). Every term SHALL read the 3D direction (no latitude or longitude), use `libm` and integer hashing only, and give the same bits for any thread count (CON-14). The constants are start values (SPIKE-92); R7 tunes them.

#### Scenario: Pure and thread-independent
- **WHEN** the continentalness of seed 14 is evaluated on 1 and on 8 threads at 2000 directions, including the poles and cube edges
- **THEN** the bits are equal

### Requirement: GEN-105 The craton layout and plate draws are retried deterministically
The continent stage SHALL draw its craton layout from the seed, then from derived seeds (draw `t` = 1 to 11 uses `seed xor (0x53927e7e * (t + 1))`, 12 draws in all), until a coarse land map (2 degree cells, 29 % of the area by the raw value's quantile, 8-connected, longitude wrapping) has 4 to 7 masses of at least 1 Mkm2 and the largest holds 47 to 60 % of the land; else it SHALL keep the draw with the lowest cost `max(0, 4 - n) + max(0, n - 7) + 10 (max(0, 0.47 - s) + max(0, s - 0.60))`, the first on ties.

Verify: A
Status: active
Source: SPIKE-92 (the layout retry), planner (2026-11-02, inbox #41); belts on the continents (round 2)

Detail: The class source SHALL draw its plates by the same kind of retry (GEN-094): up to 64 draws, draw 0 from the seed and draw `t` = 1 to 63 from `splitmix(seed xor (t << 40) xor 0x71a7e500)`, the first draw with the highest score kept. The island term (GEN-106) is part of the layout score, so changing it, or any constant of the shape, is a layout change: the kept draw of seeds 1 to 5 is pinned (draws 2, 1, 1, 2, 3 for seeds 1 to 5 and 1 for seed 14, the reference world) and a swap needs a declared re-pin. Sorts SHALL use a total order with an index tie-break; the grids and the draw order are fixed (CON-14).

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


### Requirement: GEN-094 The plate mask and the uplift fields
The default class source SHALL compute, as pure functions of the seed and the direction:

Verify: A
Status: active
Source: ADR 0089 decision 3, ADR 0088/0089 accepted 2026-11-01; SPIKE-79 (14 plates, warp 0.35), SPIKE-80 (width variety, flanks, back-arc), SPIKE-91 (belt width, segments, rings); planner critiques of the preview (2026-11-01 and 2026-11-02, inbox #39 and #40); numerics review of the preview (the jumps at plate bisectors)

Note: GEN-123's uplift field (ADR 0103) may overtake the uplift part (lead, 2026-11-14).

Detail: - **Plates:** 14 centres and Euler-pole rotation vectors (rates 0.3 to 1.0, dimensionless) from a hash of the seed and a draw index; up to 64 draws (draw 0 the seed itself, then derived seeds) are scored by the share of land (8192 points of a Fibonacci sphere) that lies near a convergent margin, `(1 - smoothstep(150, 450 km, d)) smoothstep(0.05, 0.35, convergence)` of its two nearest plates, and the first draw with the highest score is kept, so that the belts stand on the continents (GEN-105); the direction warped by 0.35 times a two-octave fBm of frequency 1.3. A plate's **land share** is the fraction of its area that is land, from 2048 points of a Fibonacci sphere assigned by the warped direction's nearest centre.
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

#### Scenario: Pure and continuous
- **WHEN** the mask is evaluated on 1 and 8 threads, at points 1e-12 either side of cube edges and corners, and at points 1e-9 rad either side of a plate bisector
- **THEN** both runs give identical bits, the points either side of a cube edge differ by under 1e-9 in every term, and those either side of a bisector by under 1e-6 in core, flank, old margin, back-arc, plateau field and `e0` in km

### Requirement: GEN-095 The default classes follow rules over the mask
The default class weights SHALL be memberships blended by smoothsteps, then normalised by GEN-093. Here `e_class` is the base elevation of GEN-096 without the hill band (GEN-098), which breaks the loop between the classes and the band, and with the ridge factor `0.5 + 0.5 r` replaced by 0.75, so that the weights do not follow the ridge noise's fine structure (GEN-097's limit on the step of `k`):
- mountains `mt = smoothstep(0, 0.4, max(core, flank))` (the membership rises over the belt's width and covers its flanks, so that the class weights change over the belt and not in one lattice step), of which high mountains `mt smoothstep(1000, 5500 m, e_class)` and mountains the rest;

Verify: A
Status: active
Source: ADR 0089 decision 3, ADR 0088/0089 accepted 2026-11-01; SPIKE-77, SPIKE-80; planner (2026-11-01): the plateau as the difference

Detail: - hills `(1 - mt) smoothstep(0.1, 0.35, max(flank, old margin, hilly))`;
- plateau `(1 - mt - hills) smoothstep(0.3, 0.6, max(plateau field, back-arc))`, so that the memberships partition 1 before the normalisation;
- the rest `1 - mt - hills - plateau`, rolling plain by `smoothstep(0.45, 0.6)` of a three-octave fBm of frequency 3.0, else plain.
Badlands and sand sea SHALL have weight 0 in the default source; class paint (ADR 0089 decision 8) or a derived source sets them. The thresholds are start values (SPIKE-80); R7 tunes them. The planet-wide check of the class shares is TEST-082.

#### Scenario: Memberships
- **WHEN** a lattice point has core 1, `e_class` 5500 m, and another has core 0, flank 0, old margin 0 and plateau field 1
- **THEN** the first holds weight 255 in high mountains and the second 255 in plateau

### Requirement: GEN-097 Amplitudes per class
Each class SHALL carry the relief driver `k`, the fine-crease floor, the carve factor (a factor on the carve's `K dt`, ADR 0089's `K t`) and the deposition's `kc`, blended by the class weights:
| Class | `k` | Crease floor | Carve factor | `kc` |
|---|---|---|---|---|
| plain | 1 | 0.2 | 1 | 1e5 |
| rolling plain | 1 | 0.35 | 1 | 1e5 |
| hills | 5 | 0 | 1 | 1e5 |
| mountains | 6 | 0 | 1 | 1e5 |
| high mountains | 7.5 | 0 | 1 | 1e5 |

Verify: A
Status: active
Source: ADR 0089 decisions 5 and 6, ADR 0088 point 1, ADR 0088/0089 accepted 2026-11-01; SPIKE-73, SPIKE-76, SPIKE-77, SPIKE-78, SPIKE-80, SPIKE-81

Detail: | plateau | 1 | 0 | 1 | 1e5 |
| badlands | 2 | 0 | 1 | 1e5 |
| sand sea | 0.35 | 0 | 0.5 | 1e5 |
- The blended `k` SHALL be tapered to 1 by `1 + smoothstep(0, 400 m, m) (k - 1)` towards the coast, with `m` the ring mean (GEN-099).
- The 99th percentile of the steps of the untapered blended `k` between neighbouring lattice points SHALL be at most 2.0 (owner, 2026-11-02: the limit applies to 99 % of the steps, not to every step); the largest step is reported, not gated. The limit is a parameter of the bake (`max_step`, default 2.0). A class paint item SHALL be feathered over at least `ceil(|delta k| / max_step)` lattice cells, so that it adds no step over `max_step`.
- The crease and detail amplitudes SHALL no longer depend on the landform driver or on the base elevation above 300 m. The erosion field's version SHALL become 3.
On seeds 1 to 5 the default source's p99 is 1.4 to 1.8 and its largest step 5.1 to 5.8 (0.4 % of the steps are over 2.0): a belt's flank changes by 0.2 to 0.3 of its core over one 40 km cell and the classes differ by 4 to 6 in `k`. Earth has 6.7 % of 40 km pairs over 2.0 (largest 5.4, p99 3.3), so the p99 form is the engine's own smoothness rule, not a realism target.
The table and the limit are start values (SPIKE-73, SPIKE-76 to SPIKE-81; `kc` from SPIKE-64); R7 tunes them on the scorecard. The planet-wide check of the step limit is TEST-082.

#### Scenario: Blended amplitudes
- **WHEN** a lattice point holds weights 128 hills and 127 mountains
- **THEN** its untapered `k` is `(128 x 5 + 127 x 6) / 255`, and at a ring mean of 0 m the tapered `k` is 1

### Requirement: GEN-098 The hill band follows the class
The hill band SHALL keep GEN-064's three octaves, weights, warped direction and coast ramp, and SHALL take the amplitude `30 + 145 smoothstep(1, 5, k) + 650 smoothstep(5, 7.5, k)` m from the untapered blended `k` (GEN-097) instead of the range zone's mountain strength. `HILL_BAND_MAX_M` SHALL become 825 m.

Verify: A
Status: active
Source: ADR 0089 decision 6, ADR 0088/0089 accepted 2026-11-01; SPIKE-80 (the hill landforms fail criterion 1 without the band), SPIKE-82

Detail: Where the class is mountains or high mountains the band SHALL be x4.5 as high (the factor is blended by the two classes' weight), its noise a ridged multifractal of five octaves (16, 8, 4, 2 and 1 km, each octave weighted by the previous one's ridge so that the ridges stay connected) in place of the three octaves, blended by the same weight, and centred (0.45 of the unit noise, its mean, is subtracted, so that the larger band does not lift the median elevation); land SHALL stay at least 1 m above the sea level where the base is (the shore of the continent stage is kept). A cap's bounds SHALL carry the band's per-octave noise bounds at the cap: for the hill octaves as before (`HILL_BAND_MAX_M`), and where the mountain weights are not 0 the ridged octaves' bound in `[0, 1]` and the factor 4.5 (the band's maximum is `0.55 * 4.5 * 825` m, about 2.0 km, and its minimum `-0.45 * 4.5 * 825` m before the land floor), all inside the clamp of 9000 m. The curve and the factors are start values (SPIKE-80; SPIKE-101, SPIKE-102 run a: x4.5 ridged centred reaches 0.79 of Everest's 3 km relief on the base); R7 tunes them. `CLASS_SOURCE_VERSION` SHALL become 6.

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

Verify: A
Status: active
Source: ADR 0088 points 1 and 2, ADR 0088/0089 accepted 2026-11-01; m9-bake PR 1 computes `m` from the existing bake; SPIKE-66 (the 10 km ring; a runtime stencil breaks batch reads and bounds)

Detail: The tapered `k` is `1 + smoothstep(0, 400 m, m) (k - 1)` of the class weights' blended driver (GEN-097). The channels are computed from the direction alone (the bake's elevation and the class weights as inputs), f64 with `libm`, stored as f32, and the same bytes for any thread count. A block's range covers the points of its cells (including the shared row and column of the next block). `RELIEF_CHANNELS_VERSION` 1 (`relief_channels.rs`).
The runtime base field SHALL be `m + k (e - m)` plus `k` times the creases (above the erosion layer's grid only the octaves GEN-109's band keeps, where the layer read calls it), with `e` the bake, each term one read at the point. It is behind a switch (`ErosionField::with_relief`), off by default:
- off: `k` is 1 and `m` is not read, today's field bit for bit, its key and implementation id unchanged;
- on: the implementation id is `cpu-erosion-v1+relief`, and the inputs hash folds the channels' identity (a hash of their content), their resolution and the channels' version; the body's relief scale SHALL be 1 (the channels are of the bake as it is), else the switch returns an error naming the scale;
- the reader (`relief_lattice.rs`) interpolates the stored f32 points in f64, continuous across face edges, takes the channels' cells per face edge from the channels (a coarse planet pass and a full one read through the same code), and returns an error naming the face for a channel with a value that is not finite.

#### Scenario: One read per term
- **WHEN** the base field is evaluated for a batch of directions
- **THEN** each direction reads `e`, `k` and `m` once at its point, and the batch equals bit for bit the heights evaluated one by one (GEN-020)

### Requirement: GEN-100 Pass 1 carves each cube face on a coarse grid
The bake's first pass SHALL, per cube face:
- sample the base field (GEN-099) once per cell of 814 m (one point sample per cell);
- carve it with implicit stream power with uplift (`U / K` 20, `K dt` 0.05 times the carve factor of GEN-097), 20 steps, not to steady state; the uplift of a cell is tapered by its starting height `h` above the sea level, `smoothstep(0, 60 m, h)` of it (a coastal plain that barely erodes is not lifted by the whole 20 m: owner 2026-11-04, the coast diagnosis);

Verify: A, B
Status: planned
Source: ADR 0088 point 4.1, ADR 0088/0089 accepted 2026-11-01; SPIKE-62, SPIKE-65, SPIKE-66, SPIKE-70, SPIKE-71

Note: a tier-B test is owed; the K7 chain (ADR 0119) overtook the bake plan (lead, 2026-11-14).

Detail: - breach after each route (the next fill sees the breached surface): from the leaves down, the receiver of a cell that stands higher than 0.1 % of the cell distance below it is lowered to that level, by at most `breach_max_m` for each donor that reaches it (500 m, SPIKE-101: with 50 m the carve fills the basins the hill band leaves and the layer halves the high mountains' relief; 0.62 of Everest's 3 km relief and 0.91 of the Alps' with 500 m), never an outlet (the sea and the face's border cells) and never below the sea level; an option `breach_height_scale_m` (off, 0) scales the limit from 50 m at the sea level to `breach_max_m` over that height of the receiver above it;
- keep the pits (GEN-120): the route runs over the filled copy, but a cell that stands below its receiver is not raised to the filled level, it only gains its uplift (`keep_pits`, on; an incise-only carve ignores it), and the face's own cells' depth below the flood of the carved heights is kept as the face's fill depth;
- keep land above the sea level (plus the flood's epsilon) whatever depth the sea outlets have;
- route with Barnes' improved priority flood (epsilon 1 mm), jittered D8 receivers (the jitter an integer hash of the GLOBAL cell and the direction, so a cell routes alike in every window that holds it) and the order from the receivers, and the drainage area by multiple flow direction (Freeman, exponent 1.1: a cell shares its area among its lower neighbours in proportion to the slope to the power 1.1, highest cells first, ties by the cell index; an outlet passes nothing on) which drives the carve while the receivers stay D8 (a D8 area imprints the grid's eight directions as staircase valleys: SPIKE-98 diagnosis, owner 2026-11-03), re-routing every 5th step; the carve factor multiplies `K dt` and the uplift (`U / K` stays 20); a height or factor that is not finite, a flood without an outlet or with an epsilon that is not positive SHALL be refused;
- not route across cube edges: each face is carved on its own grid extended by a halo of `ceil(cells / 128)` cells (at least 4) beyond each edge over the plane that continues the face, so the face's own border is an ordinary cell; the stored change keeps the halo; a reader SHALL cross-fade the faces within the halo's width of an edge (the face's own weight 1 and a neighbour's `1 - smoothstep(distance to the shared edge / halo)`, normalised; at a corner both neighbours count), so that the change is continuous across the edges and around the corners; a halo of half the face or more SHALL be refused.
It SHALL keep each face's change as a record of the layer store (STRM-065) and checkpoint per face. The constants are start values (SPIKE-62, SPIKE-65, SPIKE-66, SPIKE-70, SPIKE-71).

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

Verify: A, B
Status: active
Source: ADR 0088 points 4.2 and 6, ADR 0088/0089 accepted 2026-11-01; SPIKE-61, SPIKE-64, SPIKE-70, SPIKE-71, SPIKE-72, SPIKE-73

Detail: - the base field, evaluated on every 4th cell of the window and interpolated between by Catmull-Rom kept within the range of the four nodes around the point (the base is 86 % of a tile's cost; the stored change is taken against this interpolated window, so the interpolation error cancels in it: design.md), plus the face carve's change (read by Catmull-Rom (C1) in a face, kept within the range of the four cells around the point (a Catmull-Rom spline overshoots by up to 7.4 % of a step beside a sharp feature, which would read a one-cell channel as a levee and could put a coastal plain below the sea), since a bilinear read leaves a crease along every cell line of the pass-1 grid that the carve and the viewer would show; cross-faded within the halo of a cube edge: GEN-100);
- a local carve of catchments up to 96 cells (1 km^2 at 101.8 m) that only incises: no fill, no breach, no uplift (the routing of GEN-100, the area by multiple flow direction);
- thermal at a critical slope of 33 degrees, perturbed by up to 25 % (an integer hash of the unordered pair of global cell ids), moving 1/8 of the excess per pass (a share over 1/8 SHALL be refused: a cell would overshoot), at most 300 passes, on active cells (the cells that moved and their neighbours: bit for bit the run over every cell); a pair over the critical difference by under 1 mm does not move (the excess would never reach 0), and a cell whose height rounding leaves unchanged wakes no neighbour; the sweep keeps a cell at or below the lowest final height of its donors; land above the sea level off the grid's border gives, any cell receives;
- one deposition sweep (on its own route, whose area is by multiple flow direction too) with `kc` (1e5 uniform while every class of GEN-097 has 1e5; per class when they differ) and `zeta` 0.5 on a route of its own on the carved and thermally settled window (the load of a cell is its donors' plus its eroded column; it fills the depression first (at most to the receiver's carved height plus half the filled drop: a closed bowl's route does not become a pillar of the bowl's fill), then `zeta` of the excess over the capacity `kc sqrt(A) S` settles, at most to the filled level plus half the drop to the receiver; what reaches an outlet leaves; eroded = deposited + exported).
- a pit spreading after the sweep (`pit_min_m` 0.5 m, at most `pit_passes` 32): a land cell whose 8 neighbours are inside the grid's border, none at or below the sea level, and all higher than it by more than `pit_min_m` rises by 0.8 of its depth below the lowest neighbour while its 8 neighbours each fall by an eighth of that rise, so the sum of the heights is unchanged; the pits are taken in cell order, the pass repeats until none is deeper than `pit_min_m` (the neighbours' fall can make a smaller one), and the border cells are never lowered; serial and deterministic.
It SHALL store the core's change against the base field (STRM-065) and checkpoint per tile, with per tile the cells that still gave in the last thermal pass (the unsettled slopes at the pass cap; the pass cap is the only bound on thermal: no wall-clock budget, which would make the result depend on the machine, CON-14). The tile grid, the halo and the tiles of a run (every land tile, or a region list) are parameters of the run; the local carve runs 10 steps without uplift, breach or fill, with the drainage area capped at 96 cells (1 km^2 at 101.8 m; the cap follows the cell size, so a coarse planet pass keeps the same number of cells), and a window's cells follow the global cell of the face's grid in the jitter and the thermal's pair hash. Every kernel SHALL be a Jacobi gather or a fixed topological sweep in f64 with `libm`. The constants are start values (SPIKE-61, SPIKE-64, SPIKE-70, SPIKE-72). The seam check is TEST-084.

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

Verify: A
Status: active
Source: owner 2026-11-04 (planner inbox #58.3, M9 plan I5a(c); the one-plane, 1 m form chosen on the sizes); GEN-101

Detail: - deposited is the deposition sweep's rise plus thermal's gain, `max(0, z_final - z_thermal) + max(0, z_thermal - ref)`, `ref` the base plus the pass-1 change where it is positive (a raise by uplift or a filled pit is not a deposit);
- the depth removed is not stored: a reader derives it as `max(0, deposited - change)` with the layer's change at the cell (`removed_depth_m`); it is exact where pass 1 did not raise the cell, short by the raise where it did (15 % of mountain land and 35 % of hills in seed 14's samples), and clipped where the deposited depth is at the code's end (451.6 m); on a cell that only gained material the derived removal is at most half a step of the code plus half a step of the change (0.21 m at 1 m of deposit; the change is stored in steps of 0.25 m), so a rock or scree mask must threshold above that, not at `> 0`;
- the plane SHALL be the same bytes on any thread count.
It serves the viewer's ground materials (soil and scree where deposited, rock where the change is deep and little is deposited).

#### Scenario: The deposited depth and what is removed
- **WHEN** one synthetic tile is run with a face change of -30 m, of +10 m and with none
- **THEN** the plane holds deposits in places and none elsewhere, the codes are 0 for 0 m, 12 for 1 m, 255 from 449.8 m (decoding to 451.56 m), strictly increasing with a step of at most 0.11 m at 0.3 m, 0.12 m at 0.5 m, 0.18 m at 1 m and 0.56 m at 10 m, the round trip is within half a step in the square root, and the depth removed that is derived from a cell's deposited depth and change is never negative and is at most the pass-1 raise (zero where pass 1 did not raise) away from deposited minus change, within the code's step

### Requirement: GEN-102 Cap bounds read the layer's pyramid
The field's height bounds over a cap SHALL take the layer's part from the layer's min/max pyramid (STRM-066):
- the nodes the cap reaches, widened by 4 cells (2 for the stencil, 2 for the cross-fade at a cube edge), on every face it touches; a node wholly inside is taken whole;
- below the 32-cell blocks only for a cap of at most 8 cells' radius, down to nodes of 2 cells;
- a cap that reaches more than 36 tiles takes the layer's global range;
- the layer adds the product of that range and the coast fade's interval over the cap's base elevation (GEN-113).

Verify: A
Status: active
Source: ADR 0088 points 3 and 8, ADR 0088/0089 accepted 2026-11-01; SPIKE-82, SPIKE-83 (the steep classes plan to level 18 with no capped tile, 2.5-4 us per lookup)

Detail: With the relief channels on the bounds SHALL also take:
- the channels' range `[k_min, k_max, m_min, m_max]` over the cap: the union of the ranges of the blocks the cap touches, from a pyramid of the block ranges (a node wholly inside the cap is taken whole), and for a block the cap only touches the min and max of the lattice points of the nodes of 2 cells it touches;
- the base `m + k (e - m) + k detail` over the intervals of `e`, `m`, `k` and the detail (products of intervals, a sound bound that ignores the correlation of `e` and `m`);
- the planet-wide bound `max|1 - k| max|m| + k_max (max|e| + max|detail|)`, from `(1 - k) m + k (e + detail)`.
Because each pyramid level holds the range of the levels below and of the nodes, the bounds of nested caps SHALL nest (LOD-012). The plan check at production parameters is TEST-083.

#### Scenario: Sound and nested
- **WHEN** 20,000 random caps of radius 1e-6 to 0.01 rad over a synthetic layer are bounded, rims and face edges included, each with a nested cap inside it
- **THEN** every sampled height lies in its cap's bounds, and each inner cap's bounds lie within its outer cap's (within LOD-012's slack)

### Requirement: GEN-112 The runtime read of the erosion layer
`ErosionField::with_layer(layer, sea_level)` MAY add the stored layer (STRM-065) to a relief field; without the call the field is today's, bit for bit:
- it switches the crease band of the bake on (GEN-109) and needs no relief channels (without them the base is the lattice read at `k` 1, `m` unread);
- the layer's change is the stored change with the pit depth added back (GEN-120, until lake water and a pit-tolerance rule exist), and the pyramid (GEN-102) and the range of the manifest hold that sum;
- the height is the base (the relief base with the channels) plus the layer's change times the coast fade (GEN-113), not scaled by `k`;

Verify: A
Status: active
Source: ADR 0088 point 5 as amended by ADR 0100 (the runtime read is C1, with the coast fade and the cube-edge cross-fade; owner 2026-11-04), ADR 0098, ADR 0099; m9-bake PR 6

Detail: - the change is the monotone C1 read of the cell centres (GEN-188), within the four cells around the point, reading the neighbouring tiles' cells, clamped at a face edge and cross-faded over 2 cells with the neighbouring face's read; a tile the layer lacks is 0;
- the fine detail is pinned to the cell centres (GEN-187).

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

### Requirement: GEN-170 The channel graph as compact arrays
The bake SHALL keep the segments of H1 as compact arrays built before the discharges are released: per segment its first cell and vertex count, per vertex the direction of the step into it, the routed discharge `Q_w` as a `u16` on a log scale and the bed level as an `i16` count of 0.5 m.
- The `Q_w` code is 0 for none, else steps of equal ratio from 1e-4 to 1e7 m3/s; a read is within 1.94e-4 of the stored value and K7's burn reads the dequantised value, so the burn and the drawing use one width.

Verify: A
Status: active
Source: Architect 2026-11-10 and 2026-11-11, item 15 PR 7

Detail: - The bed level is the running minimum of the H1 surface along the segment, rounded down: the stored bed is never above the surface.
- Per segment, the vertex ranges under a lake's water are kept (the lake entry before a range, the spill cell at its end): the producer's pieces.
- An H1 cell one step outside a face resolves across the cube edge as the graph joins it; a corner quadrant has none.

#### Scenario: Chains and codes
- **WHEN** a hydrology's graph is built
- **THEN** every chain equals the routing's, every `Q_w` is within the step and every bed does not rise downstream

### Requirement: GEN-148 K7 runs on the base read, with the channels burned in
K7 SHALL start a tile from `B + C1(regional change)`, `B` read at every cell of the window (no stride) and the regional change read by a C1 read of the regional grid, burn the channels in, run the breach of pass 1 at the local grid, then the incising carve, thermal, the deposition sweep and the pit spreading, and report the change against `B`.
- The burn-in SHALL lower the cells of the 8-connected line along each segment to the segment's level profile, interpolated linearly, and SHALL NOT raise a cell.

Verify: A
Status: active
Source: A3 sections 3.1 to 3.3 and 12.1 (G-SURF), briefing item 11, inbox #104

Detail: - The pits (land cells with a fill depth of at least 0.5 m) SHALL be reported after each stage: the start, the burn-in, the breach alone, the incision, the whole pass.
- A dam across a channel SHALL be a pit before the burn-in and SHALL NOT be one after it.
- The result SHALL be the same bits on any thread count.
- The stored change SHALL read back at the centres of the cells as the bake's surface (before quantisation) and within half a step of `QUANT_STEP_M` after.
- The difference of the runtime's `B + C1(L)` and the bake's `C1(z)` at the vertices of the store lattice SHALL be reported (maximum, 50th and 99th percentile, the nodes over 0.25 m, their positions, and the two other readings: `B` plus the unclamped `C1(L)`, and the clamped read of the composed `B + L`). G-SURF is the gate at the centres of the local cells, where the bake defines its heights (owner, 2026-11-09): 0 before quantisation, at most half a step after. The vertices are the diagnostic G-SURF-I (GEN-154): 192 of 28,900 nodes (0.66 %) are over 0.25 m after quantisation on the prototype tile, maximum 2.55 m; reported, the clamp stays.
- The runs of `examples/k7_region` (the pit shares and the G-SURF numbers on a tile of the prototype region) are measurements, not gates; the gates are the tier-A tests.

#### Scenario: K7 on a tile
- **WHEN** a tile of a small planet with a valley, a dam across it and a channel along its axis is run through K7, with and without the channel, on 1 and 2 threads
- **THEN** the dam is a pit at the start and is not one after the burn-in, the channel's cells are at its profile, the bits do not depend on the thread count, and the stored change reads back at the cells' centres as the bake's surface

### Requirement: GEN-155 The sediment balance SHALL be the same under every partition the pass has
The deposition's balance per catchment SHALL be reported term by term, and the totals and the per-catchment terms SHALL not depend on how the region is partitioned.
- `catchment_terms` SHALL return for every catchment of the sweep's own route its outlet, cell count, eroded, raised and exported volumes and the relative residual of `eroded - raised - exported`; `catchment_balance` SHALL be the sum of those terms.
- The compact cube and the masked cube SHALL give the same catchments, the same terms (bit for bit, the residual included) and the same heights; the cells outside the landmass SHALL be left alone.

Verify: A
Status: active
Source: A3 section 6.7 G-SED, inbox #116

Detail: - The whole sediment pass SHALL give the same heights and report on any thread count, on a region that thermal cuts into more than two pieces.
- The sums of the terms SHALL match the eroded column read from the heights and the rise read from the volume change (relative 1e-9); the catchments, grouped by outlet, SHALL add up to the whole (1e-12) with the identity under 1e-9 in each group. This is the accounting of a cut along the divides, not a cut of the pass.
- The partition varied is stated in the test; a spatial cut of the pass is not built (open).

#### Scenario: a compact landmass under three partitions
- **WHEN** a corner-cap landmass of a sphere of 24 cells a face edge is carved for two rounds and the sediment pass runs on the compact and the masked cube, on 1, 3 and 8 threads, and its catchments are grouped by outlet id modulo 2, 3 and 5
- **THEN** the terms, the heights and the report are the same bits under the first two partitions, and under the third the groups' totals add up to the whole's with the identity at most 1e-9 in every group

### Requirement: GEN-160 K7 floods with the canonical flood by a parameter
K7 SHALL, with `canonical_flood` on, flood with the canonical flood (cells visited by level, then cell id; A3 section 6.4) in its pit statistics, its incising carve and its deposition sweep, and with it off SHALL give the bits of the plain-flood K7.
- The default pass 2 (`deposit_on_own_route`, `carve` with its default rules) SHALL be bit for bit as before.
- K7 SHALL give the same bits on any thread count with either setting.
- The change of the stored `L` against the plain flood SHALL be reported (cells changed, maximum, share over 0.25 m).

Verify: A
Status: active
Source: A3 section 6.4, inbox #114

#### Scenario: a tile with a chain beside two outlets
- **WHEN** the pit statistics and the deposition's surface are read on a flat chain between two sea cells, and K7 runs on a small tile with the plain and with the canonical flood
- **THEN** the plain statistics count the chain's lifted cells and the canonical ones none, the plain K7 equals the digest taken before this change, and the canonical K7 is the same bits on 1 and 2 threads

### Requirement: GEN-182 K7's tile layout is part of its definition
K7 SHALL be defined over a fixed tile grid of each cube face (ADR 0119): the face's `n x n` cell lattice cut into square tiles of `tile` cells an edge, tile `(ti, tj)` covering the cells `[ti x tile, (ti + 1) x tile) x [tj x tile, (tj + 1) x tile)` (the grid origin is the face's cell `(0, 0)`), each solved in a window of its core plus a halo of `halo` cells on every side. The layout SHALL be a pure function of the face lattice `n`, of `k` and of the profile, never of the request order, the region asked for, the thread count or the schedule:

Verify: B
Status: planned
Source: ADR 0119 (owner, 2026-11-12); architect D119 item 1

Detail: - `tile` divides `n`, is a multiple of the base stride (4) and is more than twice `halo`; `halo` is at most `n / 8` and, for a tile with channels, at least `DISC_RADIUS_MAX` plus the smallest profile carry (GEN-169), so that the halo holds the trench disc and the profile's carry; K7 SHALL refuse a layout that breaks any of these, naming the parameter;
- the layout's values (current value: `TileGrid` and `K7Params` as built; 128 halo cells at the decision) SHALL be inputs of K7's key, with the width law and the flood rules; a different layout is a different generator definition, a new key and no reuse of results; a change of tile size or halo is a generator change with a re-bake, never a free setting;
- for one layout, input set and version, K7's result SHALL be bit-identical under any thread count, schedule, order and worker assignment (CON-14, CON-30), and no tile's result SHALL depend on which other tiles were asked for.

At a region edge: a region SHALL be the union of whole tiles of the canonical grid, and K7's result for a tile SHALL be the result the same tile has in any other region or in the planet; a region edge adds no tile border, no halo of its own and no outlet. At a coast: cells at or below the sea level (a key input) are the outlets of a window, are neither lowered nor carry (GEN-169, GEN-176), and a window that holds sea runs the same rules as one that does not. At the window's border: the border is an outlet of the window's flood and the cells within `halo` of it are not part of the tile's result. At a cube-face edge: the halo past the face edge is the extended plane of the face (as built); how the tiles of two faces agree there is NOT yet defined and SHALL be settled with the face-seam work (REND-063) before any planet bake; the architect decides whether K7 results at face edges are canonical before then.

K7 is not validated against an unpartitioned reference (ADR 0119 point 4): CON-08's partition-invariance and border-agreement checks are replaced, for K7, by the tier B execution test (TEST-130) and the seam gate G-SEAM.

#### Scenario: One layout, many schedules
- **WHEN** K7 runs on one layout with 1, 3 and 8 threads and two schedules
- **THEN** every tile's result is the same bits

#### Scenario: A region
- **WHEN** a region is baked for a set of whole tiles and the same tiles are baked as part of a larger region
- **THEN** the results of the shared tiles are the same bits

#### Scenario: A different layout
- **WHEN** the halo or the tile size changes
- **THEN** the key changes and no stored result of the other layout is reused

### Requirement: GEN-156 The stored change L of the K7 chain is measured as kind 13 records
The measurement SHALL encode the core of the local change L of every 1,024-cell tile of the prototype region, from K7 with the channels burned in, as kind 13 counts streams and report the bytes per land cell, the spread over tiles and the extrapolation to an Earth-sized body.
- A stream SHALL decode to every cell within half a quantisation step (0.125 m) of L.
- The extrapolation SHALL name its land-cell count and say that the region is one area, not a global figure.
- The measurement SHALL NOT decide a store format; gate G-STORE is not closed by it.

Verify: D
Review: the measurement table of `openspec/changes/l-store/design.md`, judged by the architect (A3 section 12.1 gives G-STORE as a numeric gate)
Status: planned
Source: A3 section 10 (G-STORE), inbox #106 step 2

#### Scenario: the prototype region
- **WHEN** `k7_region 200 8 store` is followed by `l_measure`
- **THEN** the report lists every tile with its bytes and land cells, the pooled, median and highest bytes per land cell, and the Earth-scale size at the pooled and at the highest rate, with a round-trip error of at most 0.125 m

### Requirement: GEN-154 G-SURF is gated at the cell centres and the vertices are reported as G-SURF-I on the stored change
The surface gate SHALL read the runtime's `B + L` against the bake's surface at the centres of the local cells with `L` as the store keeps it, and SHALL report the lattice vertices as G-SURF-I without gating them.
- The gate at the centres SHALL hold: at most 1e-9 m before quantisation and half a quantisation step after.
- The stored change SHALL read back as the quantisation the gate uses (`round(L / step)` in `i16` counts times the step).

Verify: A
Status: active
Source: A3 section 12.1 (G-SURF, G-SURF-I), owner 2026-11-09, inbox #105 step 3

Detail: - G-SURF-I SHALL report the 50th, 95th and 99th percentile and the maximum of the difference at the vertices, the share over 0.25 m, the vertices and those over 0.25 m by the bake's local step height (up to 5 m, over 5 m up to 20 m, over 20 m), the largest quotient of the difference and the step height, the spatial concentration (the bins of 64 cells with vertices over 0.25 m against the bins with vertices, and the most in one bin) and the overshoot of the unclamped read of `L`.
- G-SURF-I SHALL also report the error against the local base slope (the difference over the largest rise of `B` across one cell, over the vertices whose rise is at least 1 m: 99th percentile and maximum, and their number) and the overshoot of the runtime read beyond the range of the four composed cells (maximum and the vertices over 0.25 m).
- The clamp of the C1 read SHALL stay.

#### Scenario: a smooth tile, with and without a burnt channel
- **WHEN** K7 runs on a small tile and the gate is read
- **THEN** the centres hold, every vertex is in one step class and one bin, the percentiles are ordered, and a core of change encoded as a kind 13 stream decodes to the gate's quantised values

### Requirement: GEN-164 H0 read from a stored bake equals the in-memory H0 at cube edges and corners
H0 read from a stored bake SHALL equal the in-memory H0 within the store's quantisation step at the cells of a cube edge and of a cube corner: the height of an H0 cell is the C1 read of the field at the cell's centre, the rainfall and temperature the bilinear read, with samples beyond a face edge taken from the face it meets.
- The production block codec is lossless, so the step is 0 and the two reads SHALL be equal bit for bit on every cell, every edge cell's stencil reaching over its face edge.
- A block the store does not hold SHALL make the read an error, never a placeholder.

Verify: A
Status: active
Source: Architect 2026-11-10 (item 3's stored-field read), GEN-139, GEN-107

#### Scenario: A stored bake across the cube edges
- **WHEN** a 16-cell bake in blocks of 4 is stored and the H0 cells of a 12-cell face edge are read from the store and from the in-memory bake
- **THEN** every height, rainfall and temperature of the 864 cells, 6 x 44 edge cells and 24 corner cells included, has equal bits, and a read from an empty store is an error
