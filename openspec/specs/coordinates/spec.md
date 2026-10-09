# coordinates Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: COORD-001 PlanetFixed frame
Authoritative positions SHALL be float64 vectors in the PlanetFixed frame (right-handed, +Z through the north pole, +X through latitude 0 and longitude 0, +Y through longitude 90° east), and the type system SHALL keep PlanetFixed, TileLocal and CameraRelative values distinct.

Verify: A
Status: planned
Source: CON-11, ADR 0001

#### Scenario: Frame axes
- **WHEN** geo point (0°, 0°) and (0°, 90°) and (90°, 0°) are converted at height 0
- **THEN** they equal (R, 0, 0), (0, R, 0) and (0, 0, R)

### Requirement: COORD-002 Camera-relative conversion
The CPU SHALL compute `tile_origin − camera_position` in float64 and hand the GPU only the float32 result; for a point within 10 km of the camera at any planet location the conversion error SHALL be below 1 mm.

Verify: A
Status: planned
Source: CON-11, CON-13, ADR 0001

#### Scenario: Far from the origin
- **WHEN** a camera and a point 1 m apart are placed 6,371 km from the origin
- **THEN** the float32 camera-relative offset equals the true offset within 1 mm

### Requirement: COORD-003 Geo round trip
Geo-coordinates (latitude, longitude, height) SHALL convert to PlanetFixed and back with an error of at most 1 mm everywhere on the surface, including both poles, the antimeridian and the eight cube corners.

Verify: A
Status: planned
Source: CON-13, CON-15, ADR 0011

#### Scenario: Property test
- **WHEN** a deterministic sample of geo points (including poles and the antimeridian) is round-tripped
- **THEN** the maximum position error is at most 1 mm

### Requirement: COORD-004 ReferenceSurface contract
Geo conversions SHALL go through a `ReferenceSurface` interface whose sphere implementation is the default, and every implementation SHALL pass the same contract suite (round trip, surface normal, monotone height).

Verify: A
Status: planned
Source: CON-15, ADR 0011

#### Scenario: Second implementation
- **WHEN** a new surface implementation is added
- **THEN** the shared contract suite runs against it without modification

### Requirement: COORD-005 Cube-sphere face mapping
The six cube faces (+X, −X, +Y, −Y, +Z, −Z) SHALL map face coordinates in [−1, 1]² to unit directions through the configured warp and back, with every direction belonging to exactly one face (ties at edges resolved by a fixed rule), and a round-trip direction error of at most 1e-12 radians.

Verify: A
Status: planned
Source: ADR 0003

#### Scenario: Bijection
- **WHEN** a deterministic sample of directions is mapped to (face, u, v) and back
- **THEN** each result is within 1e-12 radians of the input

### Requirement: COORD-006 Tile identifiers
A tile identifier SHALL be a 64-bit value holding face (3 bits), level (5 bits, 0 to 28) and a Morton-interleaved child path, with total functions for parent, children, level and containing tile of a direction, and decoding SHALL reject invalid values.

Verify: A
Status: planned
Source: ADR 0003

#### Scenario: Round trip
- **WHEN** any valid face, level and path are encoded and decoded
- **THEN** the same components result, and the parent of each child is the original tile

### Requirement: COORD-007 Cross-face neighbours
Neighbour lookup SHALL work across all twelve face edges and the eight face corners, SHALL be symmetric (A's east neighbour has A as its west neighbour, after the corresponding orientation change), and positions on a shared border SHALL evaluate to the same 3D direction from both sides.

Verify: A
Status: planned
Source: CON-08, ADR 0003

#### Scenario: Shared border
- **WHEN** a border sample point is evaluated from the tile on each side of a face edge
- **THEN** both directions are bit-identical

### Requirement: COORD-008 Integer hashing and noise lattices
Hash and random-number functions SHALL operate on integer inputs, be specified and versioned, and produce committed known-answer values identical on every platform.

Verify: A
Status: planned
Source: CON-14, ADR 0010

#### Scenario: Known answers
- **WHEN** the hash is evaluated for the committed input list
- **THEN** every output equals the committed value on Windows and Linux

### Requirement: COORD-009 Pure-Rust transcendentals
Code that produces tile data SHALL call `libm` for transcendental functions and SHALL NOT call the platform implementations (`f64::sin`, `cos`, `tan`, `atan2`, `exp`, `ln`, `powf` and similar).

Verify: A
Status: planned
Source: CON-14, ADR 0010

#### Scenario: Banned call
- **WHEN** a generator-relevant crate contains a banned method call
- **THEN** the scan test fails and names file and line

### Requirement: COORD-010 Vertex quantisation
Tile vertices SHALL be stored relative to the tile origin so that quantisation error at the finest level is at most 1 mm.

Verify: A
Status: planned
Source: CON-13

#### Scenario: Finest level
- **WHEN** vertices of a level-28 tile are quantised and restored
- **THEN** the maximum position error is at most 1 mm
