## MODIFIED Requirements

### Requirement: REND-003 Reversed-Z depth
Rendering SHALL use reversed-Z with a 32-bit float depth buffer, compare function greater, clear value 0 and an infinite far plane.

Verify: C
Status: active
Source: CON-12, ADR 0002

#### Scenario: Depth view
- **WHEN** the depth debug view of a near and a far object is rendered
- **THEN** the near object has the larger depth value and neither is clipped

### Requirement: REND-004 Precision at the surface
At 1 m camera distance anywhere on the planet, including 6,371 km from the origin, rendering jitter SHALL be below 0.25 px. The vertex transform (float64 camera-relative origin rounded once to f32, plus the f32 tile-local offset, through the f32 view-projection) is evaluated on the CPU in f32 exactly as the shader does and compared with the exact float64 projection; the GPU path is covered by the scripted-view tests.

Verify: A
Status: active
Source: CON-13, ADR 0001

#### Scenario: Sub-pixel motion
- **WHEN** the camera is moved in 0.1 mm steps at 1 m distance from a vertex on the far side of the planet
- **THEN** the f32 projected position stays within 0.25 px of the exact projected position

### Requirement: REND-005 No planet-scale shader values
Shaders SHALL receive only tile-local or camera-relative quantities and SHALL NOT compute values of planet-radius magnitude.

Verify: A
Status: active
Source: CON-11

#### Scenario: Review rule
- **WHEN** a shader contains a planet-radius uniform or constant
- **THEN** the shader scan test fails and names file and line

### Requirement: REND-006 Deterministic debug views
Debug views SHALL render flat, lighting-independent colours (or a grey or normal-colour ramp of data) that compare exactly against goldens of the same adapter. Provided: face, tile-id, level, morph, height, normals and depth (terrain), and face, tile-id and height (tile quads); streaming state is added with the streaming change.

Verify: C
Status: active
Source: CON-18, report §16

#### Scenario: Re-render
- **WHEN** a debug view is rendered twice on one adapter
- **THEN** the images are identical


## ADDED Requirements

### Requirement: REND-009 Terrain mesh
Each terrain tile SHALL be a regular grid whose vertices are float32 offsets from the tile origin (quantisation at the finest level at most 1 mm), with unit outward normals, a morph target on the parent grid for every vertex, skirts hanging below the border vertices, and border vertices of neighbouring tiles, also across cube faces, agreeing within float32 rounding of the tile-local offsets.

Verify: A
Status: active
Source: CON-11, CON-13, ADR 0001

#### Scenario: Parent grid
- **WHEN** a tile and its four children are meshed
- **THEN** each child's morph target equals the parent's vertex, edge midpoint or cell-diagonal midpoint it corresponds to, within 5 cm

#### Scenario: Seam
- **WHEN** two neighbouring tiles on different faces are meshed
- **THEN** every border vertex of one has a partner of the other within the rounding tolerance
