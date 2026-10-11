## MODIFIED Requirements

### Requirement: REND-009 Terrain mesh
Each terrain tile SHALL be a regular grid whose vertices are float32 offsets from the tile origin (quantisation at the finest level at most 1 mm), with unit outward normals, a morph target on the parent grid for every vertex (a position and a height), skirts hanging below the border vertices, and border vertices of neighbouring tiles of the same level, also across cube faces, agreeing within float32 rounding of the tile-local offsets in position and having equal normals (computed from a tangent frame that depends only on the vertex direction, not on a tile lattice).

Verify: A
Status: active
Source: CON-11, CON-13, ADR 0001, ADR 0003

#### Scenario: Parent grid
- **WHEN** a tile and its four children are meshed
- **THEN** each child's morph target equals the parent's vertex, edge midpoint or cell-diagonal midpoint it corresponds to, within 5 cm, and each child's coarse height equals the same rule applied to the parent's heights, within 1 cm

#### Scenario: Morphed border height
- **WHEN** a tile's border vertices are fully morphed next to a coarser neighbour
- **THEN** their coarse heights equal the coarser neighbour's own border height, linearly interpolated at that point, within 1 cm

#### Scenario: Seam normals
- **WHEN** every tile of level 2 on every face is meshed next to each of its four neighbours
- **THEN** coincident border vertices have normals that differ by less than 1e-6 degrees, across all twelve cube edges

#### Scenario: Seam
- **WHEN** two neighbouring tiles on different faces are meshed
- **THEN** every border vertex of one has a partner of the other within the rounding tolerance

### Requirement: REND-014 Vertex-shader morph
The terrain vertex shader SHALL morph each vertex from its own position towards its parent-grid position, and its height towards the parent-grid height, by the factor `clamp((d - start) / (end - start), 0, 1)`, where `d` is the camera distance to the vertex's own position and `start`, `end` are the node's morph interval from the plan; the shader SHALL read no other morph input. The `morph-height` debug view SHALL show the morphed height minus the vertex's own height over the plan's height range as the delta window.

Verify: B
Status: active
Source: CON-11, ADR 0014, ADR 0016

#### Scenario: GPU against CPU twin
- **WHEN** the morph debug view of a scripted view is rendered on the software adapter
- **THEN** at every visible interior vertex the red channel equals the f64 CPU value of the formula within the steepest neighbour step plus 2/255, and the worst error stays below 0.05

#### Scenario: Height against CPU twin
- **WHEN** the morph-height debug view of the 2,000 km and 200 km views is rendered on the software adapter with the plan's height range set to plus and minus the median nonzero height morph delta
- **THEN** at visible interior vertices whose pixel centre is within 0.15 px, the grey value equals the CPU value (morph factor times the vertex's coarse-minus-own height) within the derived neighbour bound plus 2/255, at least 60 compared vertices actually morph, and a shader that leaves the height unmorphed fails

### Requirement: REND-006 Deterministic debug views
Debug views SHALL render flat, lighting-independent colours (or a grey, colour or normal-colour ramp of data, optionally with contour lines or distance bands that make the data readable) that compare exactly against goldens of the same adapter. Provided: face, tile-id, level, morph, morph-height, height, normals and depth (terrain), and face, tile-id and height (tile quads); streaming state is added with the streaming change. The one exception to flat, lighting-independent colour is the terrain `relief` view, which exists so that frames stay legible at every altitude: a fixed-sun (wrapped, so the night side keeps a gentle gradient) hillshade of the vertex normals on a height tint, an adaptive metric grid (1 cm to 1,000 km) and height contours (1 m to 10 km), each decade fading out continuously once its lines would be closer than about 6 px. The grid is anchored to the world, not to a tile: for every decade and axis the node carries the fractional part of the tile origin over the spacing (taken in f64, so a fraction and never a planet-radius value), and the shader adds the tile-local offset in cells to it, so neighbouring tiles and LOD splits draw the same lines and the line coordinate stays small; a decade whose pixel is narrower than about 4e-7 cells fades out because an f32 fraction cannot place its lines to a fraction of a pixel. The height contours, tint and shading use the vertex height as morphed (the normal is not morphed). The view is deterministic and compares exactly like the others, but its goldens are lit views for ADR 0017.

Verify: C
Status: active
Source: CON-18, report §16

#### Scenario: Re-render
- **WHEN** a debug view is rendered twice on one adapter
- **THEN** the images are identical

#### Scenario: Readable data
- **WHEN** the height view of an aerial camera and the depth view of a ground camera are rendered
- **THEN** the height view contains contour pixels darker than the ramp and the depth view contains distance-band pixels darker than the ramp

#### Scenario: Grid anchored to the world
- **WHEN** two tiles with different origins place a grid line for the same world point
- **THEN** the shader's line coordinate (tile-local offset in cells plus the node's phase) agrees between the tiles and with the f64 world coordinate within 1 mm at every decade, and the phases are fractions in [0, 1)

#### Scenario: Ground resolution of the grid
- **WHEN** a tile near a 1 m camera, with its origin at planet-radius magnitude, places grid lines for every decade that is not faded out
- **THEN** each line coordinate is within a sixth of a pixel (1.5 mm) of the exact value

#### Scenario: Thin lines at the ground
- **WHEN** the relief view of the ground-200m and ground-1m cameras is rendered
- **THEN** less than 0.1 % of the terrain pixels are dark pixels with both a horizontal and a vertical dark run of at least 8 px (a wide band), and the same measure flags a committed image of the pre-fix relief view

#### Scenario: Relief is deterministic
- **WHEN** the relief view of an orbit and a ground camera is rendered twice on one adapter
- **THEN** the images are identical

#### Scenario: Relief keeps structure
- **WHEN** the relief view of an aerial camera is rendered
- **THEN** the terrain pixels show many distinct colours (hillshade, grid and contours), not a few flat ones
