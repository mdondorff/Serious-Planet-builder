# renderer Specification

## Purpose
The GPU side: it executes a FramePlan and decides nothing about what to draw (CON-03, ADR 0009), with reversed-Z depth (CON-12, ADR 0002), camera-relative tile-local precision with no planet-scale shader values (CON-11, CON-13, ADR 0001), the terrain mesh, vertex-shader morph and persistent terrain renderer (ADRs 0013, 0014, 0016), shader validation, deterministic debug views (CON-18) and adapter selection.

## Requirements

### Requirement: REND-001 FramePlan executor
The renderer SHALL execute a FramePlan and SHALL make no decisions about which nodes, levels, batches, tiles, rectangles or colours to draw; the number of draw calls SHALL equal the number of draws in the plan, and a plan that cannot be executed (a resource for any drawn tile is missing, in any view, rectangle outside the frame, unusable height range or resolution) SHALL be reported, not guessed.

Verify: C
Status: active
Source: CON-03, ADR 0009

#### Scenario: Plan in, pixels out
- **WHEN** a FramePlan listing two tiles is executed
- **THEN** exactly those two rectangles are drawn, the draw-call count is 2, and the GPU frame equals the CPU twin of the executor (exactly for flat views, within 1 grey level for the height view)

#### Scenario: Unexecutable plan
- **WHEN** the plan names a tile whose resource is missing
- **THEN** execution fails with the tile id

#### Scenario: Missing resource in a flat-colour view
- **WHEN** the face or tile-id view is executed with a plan that names a tile for which no resource was supplied
- **THEN** execution fails with the tile id and draws nothing; no placeholder data is substituted

### Requirement: REND-002 Shader validation
Every shipped shader module SHALL pass backend validation on the software adapter in CI.

Verify: B
Status: active
Source: ADR 0004, CON-17

#### Scenario: Invalid shader
- **WHEN** a shader module fails validation
- **THEN** the test fails and prints the shader name and the validation message

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
Debug views SHALL render flat, lighting-independent colours (or a grey, colour or normal-colour ramp of data, optionally with contour lines or distance bands that make the data readable) that compare exactly against goldens of the same adapter. Provided: face, tile-id, level, morph, morph-height, height, normals and depth (terrain), and face, tile-id and height (tile quads); streaming state is added with the streaming change. The one exception to flat, lighting-independent colour is the terrain `relief` view, which exists so that frames stay legible at every altitude: a fixed-sun (wrapped, so the night side keeps a gentle gradient) hillshade of the vertex normals on a height tint, an adaptive metric grid (1 cm to 1,000 km) and height contours (1 m to 10 km), each decade fading out continuously once its lines would be closer than about 6 px, and the contours also once the f32 height changes by less than a few of its own steps between pixels (otherwise a plateau of one height at an integer crossing draws as a wide band). The grid is anchored to the world, not to a tile: for every decade and axis the node carries the fractional part of the tile origin over the spacing (taken in f64, so a fraction and never a planet-radius value), and the shader adds the tile-local offset in cells to it, so neighbouring tiles and LOD splits draw the same lines and the line coordinate stays small; a decade whose pixel is narrower than about 8e-7 cells fades out (fully off below 4e-7) because an f32 fraction cannot place its lines to a fraction of a pixel. The height contours, tint and shading use the vertex height as morphed (the normal is not morphed). The view is deterministic and compares exactly like the others, but it has no goldens (TEST-014: the flight review and a property test cover it).

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
- **THEN** the shader's line coordinate (tile-local offset in cells plus the node's phase) agrees between the tiles and with the f64 world coordinate within 1 mm or 1e-7 cells, whichever is larger (an f32 fraction cannot do better where a cell is 100 km or more), at every decade, computed in f32 the way the shader does, and the phases are fractions in [0, 1)

#### Scenario: Ground resolution of the grid
- **WHEN** a tile near a 1 m camera (a pixel is 1.5 mm), with its origin at planet-radius magnitude, places grid lines for every decade at full strength (a pixel of at least 8e-7 cells)
- **THEN** each line coordinate is within a sixth of a pixel (0.25 mm) of the exact value, and the first scheme (a 1,000 km remainder in metres added in f32) is shown to miss it by tens of pixels

#### Scenario: Thin lines at the ground
- **WHEN** the relief view of the ground-200m and ground-1m cameras is rendered
- **THEN** less than 0.1 % of the terrain pixels are dark pixels at the centre of a 6 by 6 window that is at least 80 % dark (a wide band; thin lines and their crossings do not fill such a window), and the same measure flags a committed image of the pre-fix relief view

#### Scenario: Relief is deterministic
- **WHEN** the relief view of an orbit and a ground camera is rendered twice on one adapter
- **THEN** the images are identical

#### Scenario: Relief keeps structure
- **WHEN** the relief view of an aerial camera is rendered
- **THEN** the terrain pixels show many distinct colours (hillshade, grid and contours), not a few flat ones

### Requirement: REND-007 Adapter selection and logging
The headless harness SHALL select the software adapter for test runs (and the high-performance adapter when asked for performance runs), and SHALL log the chosen adapter's backend, name and type as its first output line; interactive adapter selection becomes a requirement when the editor gets a window (M2).

Verify: C
Status: active
Source: CON-21, report §8

#### Scenario: Test render log
- **WHEN** `planet test-render` runs on the software adapter
- **THEN** its output begins with an `adapter:` line naming backend, name and device type

### Requirement: REND-008 GPU culling differential
GPU-side culling, when added for instances, SHALL be covered by a differential test against a CPU reference on the software adapter.

Verify: B
Status: planned
Source: CON-03, ADR 0009

#### Scenario: Differential
- **WHEN** a GPU culling pass and the CPU reference run on the same instances and frustum
- **THEN** the visible sets are equal

### Requirement: REND-009 Terrain mesh
Each terrain tile SHALL be a regular grid whose vertices are float32 offsets from the tile origin (quantisation at the finest level at most 1 mm), with unit outward normals, a morph target on the parent grid for every vertex (a position and a height), skirts hanging below the border vertices, and border vertices of neighbouring tiles of the same level, also across cube faces, agreeing within float32 rounding of the tile-local offsets in position and having equal normals (computed from a tangent frame that depends only on the vertex direction, not on a tile lattice).

Verify: A
Status: active
Source: CON-11, CON-13, ADR 0001, ADR 0003

#### Scenario: Parent grid
- **WHEN** a tile and its four children are meshed
- **THEN** each child's morph target equals the parent's vertex, edge midpoint or cell-diagonal midpoint it corresponds to, within 5 cm, and each child's coarse height equals the same rule applied to the parent's heights, within 2 mm

#### Scenario: Morphed border height
- **WHEN** a tile's border vertices are fully morphed next to a coarser neighbour
- **THEN** their coarse heights equal the coarser neighbour's own border height, linearly interpolated at that point, within 2 mm

#### Scenario: Seam normals
- **WHEN** every tile of level 2 on every face is meshed next to each of its four neighbours
- **THEN** coincident border vertices have normals that differ by less than 1e-6 degrees, across all twelve cube edges

#### Scenario: Seam
- **WHEN** two neighbouring tiles on different faces are meshed
- **THEN** every border vertex of one has a partner of the other within the rounding tolerance

### Requirement: REND-010 Adaptive camera
The camera controller SHALL move at a speed proportional to the height above the terrain (within configured limits), SHALL support orbit and fly modes, SHALL never come closer to the terrain than a clearance, and SHALL be a deterministic function of its state, input and step.

Verify: A
Status: active
Source: report §10, CON-04

#### Scenario: Dive
- **WHEN** the camera descends towards flat ground
- **THEN** its speed never increases and it stops at the clearance

#### Scenario: Orbit
- **WHEN** the camera orbits at a fixed distance
- **THEN** it keeps looking at the planet centre and the distance stays constant

### Requirement: REND-011 Instance culling reference
The instance path SHALL have a CPU reference that conservatively culls bounding spheres against the four side planes of the view frustum (keeping every sphere that touches the frustum, possibly also spheres near its corners; no near or far plane), produces indirect draw arguments and camera-relative float32 instance positions, which a GPU culling pass must match (REND-008).

Verify: A
Status: active
Source: CON-03, CON-11, ADR 0013

#### Scenario: Frustum
- **WHEN** 5,000 instances are culled against a frustum
- **THEN** exactly the spheres whose centre is not farther outside a side plane than their radius are kept, in ascending order, and the indirect instance count equals their number

### Requirement: REND-012 Render graph
Rendering SHALL be described by a render graph whose passes declare reads, writes and side effects; compiling it SHALL reject a read without an earlier producer, drop passes whose outputs are unused, and compute transient lifetimes and alias groups of equal memory class.

Verify: A
Status: active
Source: report §8, ADR 0013

#### Scenario: Unused pass
- **WHEN** a pass writes a resource nobody reads and has no side effect
- **THEN** it is culled from the compiled order

#### Scenario: Missing producer
- **WHEN** a pass reads a resource that no earlier pass writes and that is not external
- **THEN** compiling fails naming the pass and the resource

### Requirement: REND-013 Persistent terrain renderer
The terrain renderer SHALL create its pipeline, uniform buffers and render targets once, keep tiles in pooled fixed-size slots of a few large vertex buffers (blocks allocated on first use, a small pool for levels 0-2, no buffer per tile), upload each tile's vertices once per mesh key into a slot (a mesh with a different key takes a new slot and the old one is released), draw a frame by writing only the per-frame uniforms and recording the draws, report CPU and wall time per frame and GPU time from timestamp queries when the device supports them, and produce exactly the image of the one-shot executor. Where a tile sits (the renderer's own free list or a placement chosen by the residency manager) SHALL NOT change the image. The bytes it allocates SHALL equal the pure model of the pool configuration (STRM-006) and a full pool SHALL refuse an upload with the tile id.

Verify: C
Status: active
Source: CON-20, CON-21, ADR 0013, ADR 0033 point 3

#### Scenario: Reuse
- **WHEN** the same tiles are drawn in several frames and views
- **THEN** no vertex data is uploaded after the first upload and every frame equals the one-shot result

#### Scenario: Missing tile
- **WHEN** a plan names a tile that was not uploaded
- **THEN** rendering fails with the tile id

#### Scenario: Changed mesh
- **WHEN** a mesh with the same tile id but a different key (other seed) is uploaded
- **THEN** the resident buffer is replaced and the next frame equals the one-shot render of the new mesh

#### Scenario: Stale mesh
- **WHEN** a plan expects a mesh key that differs from the resident buffer's key (the seed changed and the mesh was not re-uploaded)
- **THEN** rendering fails with the tile id and both keys instead of drawing the stale buffer

#### Scenario: Placement
- **WHEN** the same tiles are placed by the renderer's free list, scattered over blocks by the caller, or by the residency manager's slots across evictions
- **THEN** every frame equals the one-shot result

#### Scenario: Full pool
- **WHEN** every slot is in use or still in flight and another tile is uploaded
- **THEN** the upload fails with the tile id, and succeeds into the freed slot once a frame has retired it

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
- **THEN** at visible interior vertices whose pixel centre is within 0.15 px, the grey value equals the CPU value (morph factor times the vertex's coarse-minus-own height, as a fraction of the window) within the derived neighbour bound (interpolation share scaled by 5, because adjacent vertices alternate between morphed and not) plus 2/255, the worst error stays below a quarter of the window, at least 60 compared vertices show a morph, and a shader that leaves the height unmorphed fails
