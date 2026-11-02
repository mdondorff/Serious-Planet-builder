## MODIFIED Requirements

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

### Requirement: GEN-071 Driver channels of a tile and their blend
The driver block of a compact tile (GEN-074) SHALL hold the soil depth, the moisture and the insolation as three 8-bit codes per texel. A consumer SHALL read a driver as `neutral + w (value - neutral)` with `w` the weight of the block's value (1 in the fragment stage); the neutral values are soil 1 m, moisture 0.5 and the insolation of the latitude alone.

Verify: A
Status: active
Source: ADR 0076, planner decision 2026-10-26, ADR 0086

#### Scenario: The range edge
- **WHEN** the weight goes from 0 to 1
- **THEN** the driver goes from the neutral value to the block's value, monotone, with no step

### Requirement: GEN-072 The materials read the drivers
`MaterialParams::weights` SHALL read the soil depth, the moisture and the insolation from the tile's driver block in its reserved inputs, by the same function and the same input vector as for the other inputs.

Verify: A
Status: active
Source: LOOK-008, ADR 0076 point 5

#### Scenario: A hollow
- **WHEN** a look gives soil a curve on the moisture input and the drivers describe a wet hollow
- **THEN** soil takes weight in the hollow and none on the dry flank

### Requirement: GEN-074 The tile driver block and the driver views
A compact tile SHALL carry a driver block of 9 x 9 words (GEN-025): at each climate texel the neutral soil depth, moisture and insolation codes, and the neutral insolation of the texel's latitude in the top byte (the builder makes no other since ADR 0086). The fragment stage SHALL read the block bilinearly, blend it as `neutral + w (value - neutral)` with `w` 1 (GEN-071), and fill the reserved material inputs (GEN-072). The debug views `soil`, `moisture` and `insolation` (`ShadeView` 7 to 9, reached by J) SHALL show the blended drivers in false colour.

Verify: A, C
Status: active
Source: ADR 0076 point 5, GEN-071, GEN-072, GEN-025, ADR 0086

#### Scenario: The neutral block
- **WHEN** compact tiles are built over a field
- **THEN** every texel of the block holds the neutral codes of its latitude

#### Scenario: The views
- **WHEN** a compact tile whose driver block is uniform, and one whose block is a ramp, are drawn in each driver view
- **THEN** every covered pixel has the colour the CPU mirror gives for those codes (within 0.003 for the uniform block), and a node with no fade plane reads the block's values at full weight

#### Scenario: Constants
- **WHEN** the block is decoded
- **THEN** the soil code uses the default `soil_max_m` of 8 m in the builder and the shader alike

## REMOVED Requirements

### Requirement: GEN-036 Level-aware height

### Requirement: GEN-037 Chunk pyramid steps

### Requirement: GEN-038 Chunk border agreement and determinism

### Requirement: GEN-047 The global network in the chunk raster

### Requirement: GEN-048 The breach rule leaves no uphill channel

### Requirement: GEN-049 The halo of an A-net chunk

### Requirement: GEN-050 Jittered receivers

### Requirement: GEN-051 Diffusion and slumping

### Requirement: GEN-053 The chunk pyramid's top level

### Requirement: GEN-054 Heights of a field with chunk detail

### Requirement: GEN-057 Corridor anchors follow the drawn ground

### Requirement: GEN-058 The banks settle to the angle of repose

### Requirement: GEN-059 The bed of a river follows the ground and falls the whole way

### Requirement: GEN-060 The channel from discharge, the valley from relief

### Requirement: GEN-066 The main stem is routed over the ground

### Requirement: GEN-067 Chunks agree on the routed corridor and thread count changes nothing

### Requirement: GEN-068 The coarse run's tributaries are routed like stems

### Requirement: GEN-069 A tributary hangs above the stem and no bed climbs

### Requirement: GEN-070 Slope, curvature, wetness and moisture of a chunk

### Requirement: GEN-075 Soil depth of a chunk

### Requirement: GEN-076 Annual-mean insolation of a chunk

### Requirement: GEN-077 Routing reads known base heights

### Requirement: GEN-078 A chunk routes only the segments that can reach it

### Requirement: GEN-083 The valley band's distance is Euclidean

### Requirement: GEN-084 The routed corridor is smoothed
