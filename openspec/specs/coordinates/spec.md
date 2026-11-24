# coordinates Specification

## Purpose
Positions, frames and numeric foundations: float64 PlanetFixed positions with typed TileLocal and CameraRelative values (CON-11, CON-13, ADR 0001), the ReferenceSurface geo interface (CON-15, ADR 0011), the cube-sphere face mapping, tile identifiers and cross-face neighbours (ADR 0003), and the integer hashing and pure-Rust transcendentals that make generation deterministic (CON-14, ADR 0010).

## Requirements

### Requirement: COORD-001 PlanetFixed frame
Authoritative positions SHALL be float64 vectors in the PlanetFixed frame (right-handed, +Z through the north pole, +X through latitude 0 and longitude 0, +Y through longitude 90° east), and the type system SHALL keep PlanetFixed, TileLocal and CameraRelative values distinct.

Verify: A
Status: active
Source: CON-11, ADR 0001

#### Scenario: Frame axes
- **WHEN** geo point (0°, 0°) and (0°, 90°) and (90°, 0°) are converted at height 0
- **THEN** they equal (R, 0, 0), (0, R, 0) and (0, 0, R)

### Requirement: COORD-002 Camera-relative conversion
The CPU SHALL compute `tile_origin − camera_position` in float64 and hand the GPU only the float32 result; for a point within 10 km of the camera at any planet location the conversion error SHALL be below 1 mm.

Verify: A
Status: active
Source: CON-11, CON-13, ADR 0001

#### Scenario: Far from the origin
- **WHEN** a camera and a point 1 m apart are placed 6,371 km from the origin
- **THEN** the float32 camera-relative offset equals the true offset within 1 mm

### Requirement: COORD-003 Geo round trip
Geo-coordinates (latitude, longitude, height) SHALL convert to PlanetFixed and back with an error of at most 1 mm everywhere on the surface, including both poles, the antimeridian and the eight cube corners.

Verify: A
Status: active
Source: CON-13, CON-15, ADR 0011

#### Scenario: Property test
- **WHEN** a deterministic sample of geo points (including poles and the antimeridian) is round-tripped
- **THEN** the maximum position error is at most 1 mm

### Requirement: COORD-004 ReferenceSurface contract
Geo conversions SHALL go through a `ReferenceSurface` interface whose sphere implementation is the default, and every implementation SHALL pass the same contract suite (round trip, surface normal, monotone height).

Verify: A
Status: active
Source: CON-15, ADR 0011

#### Scenario: Second implementation
- **WHEN** a new surface implementation is added
- **THEN** the shared contract suite runs against it without modification

### Requirement: COORD-005 Cube-sphere face mapping
The six cube faces (+X, −X, +Y, −Y, +Z, −Z) SHALL map face coordinates in [−1, 1]² to unit directions through the configured warp and back, with every direction belonging to exactly one face (ties at edges resolved by a fixed rule), and a round-trip direction error of at most 1e-12 radians.

Verify: A
Status: active
Source: ADR 0003

#### Scenario: Bijection
- **WHEN** a deterministic sample of directions is mapped to (face, u, v) and back
- **THEN** each result is within 1e-12 radians of the input

### Requirement: COORD-006 Tile identifiers
A tile identifier SHALL be a 64-bit value holding face (3 bits), level (5 bits, 0 to 28) and a Morton-interleaved child path, with total functions for parent, children, level and containing tile of a direction, and decoding SHALL reject invalid values.

Verify: A
Status: active
Source: ADR 0003

#### Scenario: Round trip
- **WHEN** any valid face, level and path are encoded and decoded
- **THEN** the same components result, and the parent of each child is the original tile

### Requirement: COORD-007 Cross-face neighbours
Neighbour lookup SHALL work across all twelve face edges and the eight face corners, SHALL be symmetric (A's east neighbour has A as its west neighbour, after the corresponding orientation change), and positions on a shared border SHALL evaluate to the same 3D direction, bit for bit, from both sides (guaranteed by evaluating face lattice samples with exact edge values and an odd warp, see design); corner neighbours (the other tiles sharing a tile corner) SHALL be three at an ordinary corner and two at each of the eight cube corners, and symmetric

Verify: A
Status: active
Source: CON-08, ADR 0003

#### Scenario: Corner neighbours
- **WHEN** the corner neighbours of every tile corner of levels 1 to 3 are listed
- **THEN** each list equals the set of other tiles with a bit-identical corner direction, has 3 entries (2 at the 24 cube-corner cases per level) and is symmetric

#### Scenario: Shared border
- **WHEN** a border sample point is evaluated from the tile on each side of a face edge
- **THEN** both directions are bit-identical

### Requirement: COORD-008 Integer hashing and noise lattices
Hash and random-number functions SHALL operate on integer inputs, be specified and versioned, and produce committed known-answer values identical on every platform.

Verify: A
Status: active
Source: CON-14, ADR 0010

#### Scenario: Known answers
- **WHEN** the hash is evaluated for the committed input list
- **THEN** every output equals the committed value on Windows and Linux

### Requirement: COORD-009 Pure-Rust transcendentals
Code that produces tile data SHALL call `libm` for transcendental functions and SHALL NOT call the platform implementations (`f64::sin`, `cos`, `tan`, `atan2`, `exp`, `ln`, `powf`, `powi`, `mul_add` and similar, on `f64` and `f32`, in any call form). The ban SHALL be enforced over the whole workspace by clippy `disallowed-methods` in `cargo xtask check`, with a text scan as a fallback.

Verify: A
Status: active
Source: CON-14, ADR 0010

#### Scenario: Banned call
- **WHEN** a crate contains a banned method call in any form (method, path, `<f64>::` or as a function value)
- **THEN** `cargo xtask check` fails with the offending file, line and the libm replacement, and the scan test fails for the forms it covers

### Requirement: COORD-010 Vertex quantisation
Tile vertices SHALL be stored relative to the tile origin so that quantisation error at the finest level is at most 1 mm.

Verify: A
Status: active
Source: CON-13

#### Scenario: Finest level
- **WHEN** vertices of a level-28 tile are quantised and restored
- **THEN** the maximum position error is at most 1 mm

### Requirement: COORD-011 Global cells and the cross-edge fold
`planet_core::GlobalCell` SHALL name a cell `(face, i, j)` of a level with `n` cells per face edge (`0 <= i, j < n`) as one `u64`, and its neighbours SHALL cross the twelve cube edges by the public `planet_core::tile::fold_cell` (exact integers on the cube, the fold `TileId::neighbors8` uses): an axis step that leaves a face lands on the cube-adjacent cell of the neighbour face, and no diagonal crosses an edge or a corner (GEN-139).

Verify: A
Status: active
Source: Lead 2026-11-24 (#193, owner O2 2026-11-23: planet primitives on main during G3, additive and output-neutral; the G3 port order's C1), GEN-139, COORD-007
Detail:
- bits: face in bits 60-62, `j` in bits 30-59, `i` in bits 0-29; 30 bits an axis hold 2^30 cells per face edge (`MAX_CELLS_PER_EDGE`), above the runtime's deepest level; bit 63 stays reserved (0); a cell outside `0..n`, a face over 5 or an `n` over 2^30 is refused; the id is private, built only by `new` or `from_raw` (which refuses a face over 5 or bit 63), and read by `raw`;
- order: the `u64` order is (face, j, i); it is the canonical tie key of the seam-pair spill engine (GEN-191);
- `step(n, di, dj)` for `|di|, |dj| <= 1`: inside the face the cell itself; one coordinate one step outside `0..n` with the other inside and an axis step, `fold_cell`; a diagonal that leaves the face, or both coordinates outside, has none; a cell outside the level `n` passed in has none (`in_level`; `neighbours4` panics on it, a caller's error);
- `neighbours4(n)` (east, west, north, south) always exist; `neighbours8(n)` adds the diagonals that stay in the face: 8 inside a face, 6 on an edge, 5 at a face's corner cell (4 at `n = 1`);
- `centre_point(n)`: the cell centre as a cube point in half-cell units (normal coordinate `n`, in-face coordinates `2 i + 1 - n`, `2 j + 1 - n`); `of_centre_point(p, n)` its inverse, None for a point off the cube's surface or a level out of range;
- additive: no caller changes; hashes, goldens and the determinism manifest stay bit-identical.

#### Scenario: Rotations and corners
- **WHEN** every cell of levels with 4 and 5 cells per face edge is mapped by each of the cube's 24 rotations (its centre point rotated)
- **THEN** its 4- and 8-neighbourhoods map onto the rotated cell's, every neighbour relation is symmetric, a neighbour on another face is an axis neighbour, and at each cube corner the three corner cells are pairwise axis neighbours

### Requirement: COORD-012 A tile plus its halo as one grid on the cube
`planet_core::CubePatch` SHALL be a square core of `size x size` cells of one face of a level with `n` cells per face edge plus a halo of `halo` cells on every side, as one grid of `(size + 2 halo)^2` cells `(u, v)`, `-halo <= u, v < size + halo`, each a `GlobalCell` (COORD-011), so that ties between its cells break canonically (CON-30). It is the only tile-plus-halo grid the L stages use (reconciliation row 7, from the port on).

Verify: A
Status: active
Source: Lead 2026-11-24 (#195, owner O2 2026-11-23: planet primitives on main during G3, additive and output-neutral; the G3 port order's C2, reconciliation rows 1, 2 and 7), COORD-011, GEN-139, CON-30
Detail:
| Patch cell `(i, j) = (i0 + u, j0 + v)` | Its `GlobalCell` |
|---|---|
| inside the face | the face's cell `(i, j)` |
| one coordinate outside `0..n` | the neighbour face's cell folded over that edge (`fold_cell`'s rule, any depth up to `n`; axis-only, GEN-139) |
| both outside (past a cube corner) | two folds, the deeper coordinate first; at equal depths, first across the edge towards the face `F` with `det(n, n_F, n_other) > 0` |
- refused (None): a core outside the face, `size < 1`, `halo < 0` or `halo > n`, `n` outside `1..=2^30`;
- the stitched quadrant past a cube corner continues each strip across the corner's third edge; its cells are cells of the patch's strips (a cube corner has 270 degrees), so a corner patch has no hole and holds no cell of a face outside the corner's three;
- `extended_point(u, v)`: the cell's point on the core face's extended plane in half-cell units; a rotation of the cube maps it onto the rotated patch's;
- additive: no caller changes; `fold_cell` keeps its results (its body is `fold_cell_axis`, crate-private); hashes, goldens and the determinism manifest stay bit-identical.

#### Scenario: Interior, halo and corner
- **WHEN** every core position of cores of 2 to 4 cells with halos of 2 to 5 is built on every face of levels of 6 to 12 cells
- **THEN** inside the face each cell is the face's own; each cell across an edge is the cell reached by straight axis steps from the face's border; each stitched cell past a corner is the straight continuation of the strip it adjoins (on the diagonal one of the two); a corner patch spans three faces and every stitched cell repeats a strip cell

#### Scenario: Rotations
- **WHEN** every patch of levels of 6 and 7 cells is mapped by each of the cube's 24 rotations
- **THEN** the rotated patch holds, at the rotated extended point of each cell, the rotated cell
