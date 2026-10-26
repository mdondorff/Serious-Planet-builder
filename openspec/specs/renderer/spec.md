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
Geometry shaders (terrain, tile quads, windows) SHALL receive only tile-local or camera-relative quantities and SHALL NOT compute values of planet-radius magnitude: no planet-radius uniform or constant. The atmosphere's shaders (the sky-view and aerial-perspective compute passes, the composite, and the terrain's atmosphere lighting in the fragment stage) are presentation shading (CON-11 v1.4, ADR 0043, ADR 0050 point 6): they MAY hold the body's radius as a value in their uniform block, decided in f64 on the CPU, and form ray quantities by the altitude-relative primitives; they SHALL contain no planet-scale literal or radius-like name, no vertex stage SHALL read that block (the scan asserts it), and their declared bound (ATMO-001, ATMO-010) is tested.

Verify: A
Status: active
Source: CON-11, ADR 0043

#### Scenario: Review rule
- **WHEN** a shader contains a planet-radius constant or a geometry shader a planet-radius uniform
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
The terrain renderer SHALL create its pipeline, uniform buffers and render targets once, keep tiles in pooled fixed-size slots of a few large storage-buffer blocks (allocated on first use or by the warm-up of REND-017, no buffer per tile), upload each tile's slot (GEN-025) once per mesh key (a mesh with a different key takes a new slot and the old one is released), draw a frame by writing only the per-frame uniforms and recording the draws, report CPU and wall time per frame and GPU time from timestamp queries when the device supports them, and produce exactly the image of the one-shot executor. Where a tile sits (the renderer's own free list or a placement chosen by the residency manager) SHALL NOT change the image. The bytes it allocates SHALL equal the pure model of the pool configuration (STRM-006) and a full pool SHALL refuse an upload with the tile id.

Verify: C
Status: active
Source: CON-20, CON-21, ADR 0013, ADR 0033 point 3

#### Scenario: Reuse
- **WHEN** the same tiles are drawn in several frames and views
- **THEN** no tile data is uploaded after the first upload and every frame equals the one-shot result

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
The terrain vertex shader SHALL morph each vertex from its own position towards its parent-grid position, and its height towards the parent-grid height, by the factor `clamp((d - start) / (end - start), 0, 1)`, where `d` is the camera distance to the vertex's own position and `start`, `end` are the node's morph interval from the plan; the shader SHALL read no other morph input. The CPU twin is checked on the reference renderer's copy of the shader (REND-031: the same WGSL head and tail as the compact shader, with the vertex stage reading a vertex buffer); the compact vertex stage's morph is checked against the reference by the differential of REND-016 with every vertex morphed to the parent grid. The `morph-height` debug view SHALL show the morphed height minus the vertex's own height over the plan's height range as the delta window.

Verify: B
Status: active
Source: CON-11, ADR 0014, ADR 0016

#### Scenario: GPU against CPU twin
- **WHEN** the morph debug view of a scripted view is rendered on the software adapter
- **THEN** at every visible interior vertex the red channel equals the f64 CPU value of the formula within the steepest neighbour step plus 2/255, and the worst error stays below 0.05

#### Scenario: Height against CPU twin
- **WHEN** the morph-height debug view of the 2,000 km and 200 km views is rendered on the software adapter with the plan's height range set to plus and minus the median nonzero height morph delta
- **THEN** at visible interior vertices whose pixel centre is within 0.15 px, the grey value equals the CPU value (morph factor times the vertex's coarse-minus-own height, as a fraction of the window) within the derived neighbour bound (interpolation share scaled by 5, because adjacent vertices alternate between morphed and not) plus 2/255, the worst error stays below a quarter of the window, at least 60 compared vertices show a morph, and a shader that leaves the height unmorphed fails

### Requirement: REND-015 Indirect multi-draw
The terrain renderer SHALL be able to submit a plan's draws as indirect multi-draws built from the plan on the CPU: one `DrawIndexedIndirect` record per planned node in the plan's order (`first_instance` = the node's index, `base_vertex` 0, because a compact tile has no vertex buffer and the vertex stage pulls from the block (ADR 0054), `instance_count` 1, the index count of the tile grid), grouped into runs of consecutive nodes that sit in one block of the pool, one `multi_draw_indexed_indirect` per run, with the per-node records in a storage buffer the shader indexes with the instance index. The per-draw loop SHALL remain, selectable (`--draw per-draw|indirect`), as the fallback where the adapter lacks `INDIRECT_FIRST_INSTANCE` or indirect execution, and as the reference: both paths SHALL draw identical images for the same plan. The records SHALL come from the plan and the slots alone (no GPU-side culling or decisions, CON-03); per-frame globals, node records and argument records SHALL live in a ring so a frame in flight is never overwritten (STRM-016); shaders SHALL read only camera-relative and tile-local values (REND-005). The bytes written per frame for node and argument records SHALL be reported (TEST-011).

Verify: A, C
Status: active
Source: CON-03, CON-11, ADR 0033, M4 item 7 (review item C)

#### Scenario: Records equal the plan
- **WHEN** the draw list of a standard view's plan is built from the slots of its tiles
- **THEN** there is one record per node in the plan's order, `first_instance` equals the node index, `base_vertex` is 0, the runs cover the records, and the bytes per frame (208 B node record, the 192 B of origin, colour, morph interval and grid phases and the 16 B up vector of ADR 0039, plus 20 B argument record per node) stay within nodes x 228 B (the pin moved from 212 with the record) for each of the ten views (a declared addition to the pinned counts)

#### Scenario: Identical images
- **WHEN** the ten scripted views are rendered in the level and relief views, with the tiles placed by the free list and scattered over blocks, once per-draw and once indirect, on the software adapter and on the discrete GPU
- **THEN** the images are identical and the renderer reports the same draws, triangles and node-record bytes, one argument record per node, and one multi-draw per run (more than one where the nodes sit in several buffers)

#### Scenario: Unsupported adapter
- **WHEN** indirect mode is requested on an adapter without the feature
- **THEN** the renderer keeps the per-draw loop and reports the mode in effect

#### Scenario: Frames in flight
- **WHEN** more frames than the ring is deep are submitted without waiting, alternating two plans
- **THEN** the last frame shows its own plan, and `--stream` and `--pipelined` runs on either path settle on the same image

### Requirement: REND-016 Compact tile rendering
The terrain renderer SHALL hold every drawn tile, levels 3 and finer (the render floor, LOD-011), in the compact format (GEN-019), the only tile format, in pooled slots of storage-buffer blocks and draw them with a vertex stage that places each vertex from the texels of its tile and of the neighbours the morph needs (the exact morph target). A block of the main pool SHALL be one storage buffer of at most 128 MiB (`block_slots x slot_bytes`: 14,146 slots of 9,488 bytes at cells 32, allocated on first use or by the warm-up of REND-017), its slots following one another and the texels row-major in the slot (the slot of GEN-025: a 128-byte header, the 324-byte climate block, the driver block and wide texels, a 32-bit height step count and two 15-bit octahedral normal components; `MESH_VERSION` 10, so every terrain golden is regenerated through `golden-review`); the vertex stage SHALL read the slot's header and the texel `j x (cells + 1) + i` at the offsets of GEN-025 from the block bound for the run and decode the height as `hr_min + f32(q) * hr_step` and the normal by `extractBits`, and the fragment stage the slot's climate block (GEN-021) and driver block; there is no separate record buffer; a draw SHALL name its block by being in a run of that block (consecutive nodes in one block form one run, in the plan's order) and its slot through a per-frame 16-byte tile reference (the slot's first word in its block, reserved, the slot index, reserved) indexed by the instance index (`first_instance` = the node index). The per-draw loop SHALL remain the reference: both submission paths SHALL draw identical images for the same plan whatever the slots, and the image SHALL equal the reference renderer's (REND-031: the same plan drawn from the CPU meshes) within the model tolerance of the compact tile (position model error, 15-bit normals): in the level view at most 0.05 % of the pixels differ by more than 12 levels, in the relief and normals views at most 0.2 %, and on the steep stress field at most 1 %. With every vertex morphed to the parent grid the images SHALL still agree, so a wrong morph target in the shader fails. A tile below the render floor (a compact tile of level 2 or coarser) or a slot outside the pool SHALL be refused, not written. There is no fallback form: a tile whose relief the wide texel cannot hold is a generator concern (GEN-019), never drawn in another format. The product paths (`test-render` single views, `--stream`, `--pipelined`, `--flight`, `--perf`, the repro replay) SHALL build and draw the compact format only (there is no `--tile-format`); a flight SHALL equal the reference renderer's frames of the same plans within 0.1 % of the pixels over 12 levels. Shaders SHALL read only camera-relative and tile-local values (REND-005 scans the compact shader for every tile size). The allocation SHALL equal the pure model of the pool configuration: compact blocks (buffer bytes), index buffer, and node records, and the mid and high shares SHALL hold 15 and 23 blocks (212,190 and 325,358 slots at cells 32, STRM-006). The known-answer pixel hashes of the buffer path (the ten scripted views in every debug view, persistent renderers per slot and batched, the lit frame with the atmosphere, the lit water frames) SHALL be re-recorded for the wide texel under `MESH_VERSION` 10 on WARP and on the discrete GPU, each change through `golden-review` as rounding noise (ADR 0017), and the pixels SHALL equal them.

The node record's level lane SHALL carry, 256 times, the edge masks of the draw (ADR 0074: `stitch` in the low four bits, `fine` above them; 0 for a node with none, so its record and its pixels are what they were). The vertex stage SHALL morph a vertex on a stitched edge fully to the parent grid and one on a fine edge not at all, whatever its distance, the skirt vertices following the edge vertex they hang from (the vertex's grid coordinates `i` and `j`: east `i = cells`, west `i = 0`, north `j = cells`, south `j = 0`); the stale-tile overlay reads the state from the lane modulo 256. The test-only reference renderer, which morphs by the vertices of a CPU mesh, SHALL refuse a masked node (`ExecError::MaskedNode`).

Verify: A, C
Status: active
Source: ADR 0033 points 3-5, ADR 0054, SPIKE-06, CON-03, CON-11, GEN-019, GEN-021, REND-015, GEN-025, ADR 0062

#### Scenario: Images against the vertex path
- **WHEN** the ten scripted views are rendered in the level, relief and normals views from compact tiles and by the reference renderer from the meshes, on the software adapter (cells 16), and three views at cells 32 including the 1 m ground camera
- **THEN** the pixels that differ by more than 12 levels stay within the stated fractions, and compact tiles were drawn

#### Scenario: Default path and flight
- **WHEN** the scripted flight is rendered by the product path and, from the same plans, by the reference renderer
- **THEN** every frame differs in at most 0.1 % of its pixels by more than 12 levels; the golden views drawn from compact tiles agree with their goldens within rounding on the adapters that have them

#### Scenario: Morph target
- **WHEN** every node's morph interval is forced so each vertex is drawn at its parent-grid position and height, on the steep field, in the relief and morph-height views
- **THEN** the compact and reference images agree within the steep tolerance (the morph-height view shows variation, and a shader that morphs the height to the wrong target fails)

#### Scenario: Both submission paths, any placement
- **WHEN** a plan is drawn per-draw and indirect with the tiles placed by the free list and scattered over blocks
- **THEN** all four images are identical, and the scattered indirect path needs more than one multi-draw

#### Scenario: Fallback and coarse tiles beside compact tiles
- **WHEN** the steep stress field is planned with the finest level declared at 9 so finest tiles exceed 65.535 m of relief
- **THEN** every drawn tile is compact with the wide texel, none of level 0-2 is drawn, and the image equals the reference renderer's within the steep tolerance

#### Scenario: Pools and the share
- **WHEN** the tier pools are configured for cells 32 (and a real block of 128 MiB is drawn from, with tiles in its last slots)
- **THEN** the mid share holds 15 blocks (212,190 slots) and the high share 23 (325,358), the model's allocation stays under the share with the fixed items inside their reserve, a renderer's allocation equals the model for its live blocks, and no tile size overflows the reserve with records (the record is in the slot)

#### Scenario: The buffer path draws the recorded pixels
- **WHEN** the ten scripted views are drawn in all nine debug views from compact tiles (one-shot), persistent renderers draw four views per slot and batched, and the lit frame with the atmosphere and the lit water frames are drawn
- **THEN** the hash of the pixels equals the hash recorded for the wide texel under `MESH_VERSION` 10 for the adapter's driver (WARP, the discrete GPU), and a driver without a recorded hash prints its hash

#### Scenario: One slot, one write
- **WHEN** a tile is written to a block and the block is read back
- **THEN** the slot's bytes at its start equal the CPU tile's slot (GEN-025) from one `write_buffer` range, no record buffer exists, and the shader's header, climate and driver reads equal the CPU mirror's

### Requirement: REND-017 No one-time initialisation inside a measured frame
The terrain renderer SHALL be able to do the one-time work of its first frames before the first frame (`warm_up`): create the first block of the pool (the storage buffer of block 0 with its bind group), grow the per-frame ring to a count above the node count of the standard scenes' plans (2048 nodes, within the share's node-record bytes), and submit one throwaway frame, polled to completion, that is representative of the measured frames: it records from every set of the per-frame ring, draws a zeroed tile through every pipeline of the tile format in the active draw mode (the indirect path as a multi-draw with a count above one), and runs the timestamp pass with its resolve and readback where the adapter supports timestamps, so that the lazy first-use work of the driver and of wgpu (pipeline state, resource initialisation) is done outside any measured frame. The throwaway frame SHALL make no tile resident, SHALL NOT count as a frame (the frame clock of the slot retirement and the frames in flight do not move) and SHALL NOT change any image. The bytes it allocates SHALL equal the pure model of the pool configuration with one block live (STRM-006); every other block and ring growth stays on demand and the share stays a ceiling. The renderer SHALL count the long-lived GPU objects it creates (pipelines, pool allocations, ring builds); after `warm_up`, a script whose first frames fit the warmed block and ring SHALL create none in frames 0 and 1. The renderer SHALL also count the parts of itself a submission has used (ring sets, pipelines, timestamp passes, draw modes, lit variants); after `warm_up` these SHALL be the maximum a scene's frames reach, and frames 0 to 3 (untimed and timed) SHALL use no part for the first time. `enable_lit` and `enable_atmosphere` are not part of `warm_up`: `warm_up_lit` SHALL enable the lit path and, for a plan with an atmosphere, the atmosphere, and draw that plan (no node) once for every set of the ring, each polled to completion, after which lit frames with tiles, adaptive and with the atmosphere, SHALL use no part for the first time and create no long-lived object; the lit variants recorded (a lit frame, a lit frame with the atmosphere) are counted. The pipelined loop, the streaming perf scenes and the viewer window SHALL warm the renderer in their setup, outside the timed loop; frame 0 stays in the measured frames (ADR 0032, TEST-018) and records like the frames after it.

Verify: C
Status: active
Source: ADR 0032, REND-013, REND-016, TEST-018, STRM-006, perf windows 1 and 2 (F4, F9), owner 2026-10-17 (item 5b)

#### Scenario: Nothing is created in the first frames
- **WHEN** a renderer, per-draw and indirect, is warmed and then uploads a plan's tiles and submit frames 0, 1 and 2
- **THEN** the pipeline, pool-allocation and ring-build counters and the live blocks are the warm-up's, none became resident during the warm-up, and a renderer that was not warmed allocates in the same frames

#### Scenario: Allocation equals the model
- **WHEN** a renderer is warmed
- **THEN** one block is live, and `pool_allocated_bytes` equals `allocated_bytes_with(1)` with the node records at their allocated size, under the share

#### Scenario: Same image
- **WHEN** the same plan is rendered by a warmed and by a cold renderer
- **THEN** the pixels are identical

#### Scenario: Lit warm-up
- **WHEN** a warmed renderer is given `warm_up_lit` with an atmosphere plan and then draws four lit frames with tiles, untimed and timed
- **THEN** the lit and atmosphere variants are both counted after the warm-up, the other counters and the long-lived lit objects do not change in the four frames, and no tile became resident during the warm-up

#### Scenario: Nothing is first-used by a measured frame
- **WHEN** a renderer, per-draw and indirect, is warmed, then submits frames 0 to 3 alternating untimed and timed
- **THEN** the warm-up used every ring set, every pipeline of the format, the timed pass where supported and the active draw mode in one waited submission, frames 0 to 3 change none of those counts, and a cold renderer reaches every ring set only through its first two frames

### Requirement: REND-018 Batched tile writes
The terrain renderer SHALL write a frame's tiles to the pools in batches: `TerrainRenderer::upload_tiles_at` places the frame's tiles in the order given (the upload order of STRM-006), each exactly as `upload_tile_at` does (the manager's slot, the same checks, errors and bookkeeping), and issues the data writes of the compact tiles after the last one, sorted by slot, with one `write_buffer` per run of adjacent slots of one block for the slots (the run's slots concatenated into one contiguous range of the block: header, climate block and texels, GEN-025); a slot written twice in a batch SHALL hold the last tile. The calls a batch makes for compact tiles SHALL be exactly the number of those runs (slot runs), so at most the per-slot path's one per tile for any slots, and for adjacent slots of one block one run. The reference path (`UploadMode::PerSlot`: `--upload per-slot`, one `write_buffer` call per compact tile) SHALL stay selectable, the default being batched, and the pool contents (every block, read back), the images and the bookkeeping SHALL be identical in both modes. The renderer SHALL count the queue calls (`write_calls`) and the bytes handed to them (`staging_bytes`) of its tile writes; the managed loop SHALL hand the frame's tiles to its sink as one batch (`TileSink::upload_batch`, whose default is the loop over `upload_at`, one call per tile, so a sink that does not batch keeps working), in the upload order, and report the sink's calls in the frame's counts (`upload_calls`, TEST-012). The tile bytes of the 1 MiB cap (STRM-006) and the placement of the manager SHALL NOT change. The compact blocks carry `COPY_SRC` so that tests can read the pools back (`read_compact_pools`; no change to the allocated bytes). The conversion of a tile to bytes (texels) SHALL NOT go through a byte-wise iterator (about eight times the CPU of copying the same bytes in slices). The staging of a batch SHALL NOT allocate per frame once the buffers have grown to the largest batch: the converted texels, the lists of writes, their sort orders, the rectangle of a run live in buffers the renderer keeps and clears between batches (`TerrainRenderer::batch_scratch_capacity` stays constant over frames of the same or a smaller size), because the frame thread shares the heap with the generation threads and the time of an allocation there varies with their load. The managed loop SHALL keep its list of waiting tiles and its batch between frames, SHALL rebuild the draw class of the upload ranking (the draw tiles with their ancestors and plan priorities) only when the draw wants changed and the prefetch closure only when the prefetch wants changed (`ManagedLoop::ranking_rebuilds`), and SHALL rank the residency class by looking the tile up in R, which is closed under parents, never by building a set over R (R is sorted and closed under parents, `ResidencySet::is_sorted_and_closed`, which the loop checks in debug builds; it costs the size of R, about 11,000 tiles at the ground), so that a frame whose wants stayed builds no set over them whatever it uploads, and the draw walk SHALL stop at an ancestor already holding a priority at most the walked tile's (the sets equal those of the full walk), so that the ranking costs the draw set's size and not its size times the depth.

Verify: A, C
Status: active
Source: CON-20, CON-21, STRM-006, REND-013, REND-016, ADR 0033 point 9, ADR 0054, TEST-018, perf window 1 (F5), GEN-025, ADR 0062

#### Scenario: Calls follow runs
- **WHEN** a frame writes 1, 10 and 118 compact tiles into adjacent slots, from the middle of a row, every third slot, and in a shuffled order
- **THEN** the batched path makes the number of runs of adjacent slots in calls (118 adjacent tiles over blocks of 64 slots: 2 runs, 2 calls against 118 per slot; inside one product block, 1 call), never more than the per-slot path, and the same staging bytes

#### Scenario: Same pools and images
- **WHEN** the same tiles are placed per slot and batched, including a slot written twice in one batch and the ten standard views
- **THEN** every block read back equal, the images are identical, and a batch that fails writes the tiles placed before the error

#### Scenario: The loop hands over one batch
- **WHEN** the managed loop runs the ground fill with caps of 1, 10 and 118 tiles
- **THEN** a frame that writes calls `upload_batch` once with its tiles in the order the per-slot path wrote them, in the same slots, with the same counts and bytes, `upload_calls` is the sink's calls (one per tile for a sink that does not batch, the runs of adjacent slots for one that does), and a frame that writes nothing makes no call

#### Scenario: Pipelined loop on the renderer
- **WHEN** the pipelined loop runs the cold start and a jump with `--upload per-slot` and `--upload batched` on the software adapter
- **THEN** the settled frames are identical and the batched log's `upload_calls` per uploaded tile is lower

#### Scenario: Staging buffers are reused
- **WHEN** frames of 118, 10, 20 and 100 compact tiles in moving, reversed and shuffled slots are placed one after another on one renderer
- **THEN** the capacity of its staging buffers is zero before the first batch, equal from the second frame on, and the pools read back equal those of the per-slot path that wrote the same frames

#### Scenario: Ranking sets follow the wants
- **WHEN** the managed loop uploads a fill of hundreds of tiles at two a frame with a camera that stays
- **THEN** no frame whose draw wants equal the previous frame's rebuilds the draw class, no frame whose prefetch wants stayed rebuilds the prefetch closure, and the draw class built with the early-stopping walks equals that of the definition for ascending, shuffled and tied priorities

### Requirement: REND-019 Lit frame graph
The lit path SHALL be declared as a render graph in `render::graph` (`lit_frame_graph`, a function of the frame size and the exposure mode) with the passes `terrain` (writes the scene target `hdr` and `depth`), `exposure-histogram` (reads `hdr`), `exposure-adapt` (reads the histogram only under adaptive exposure; writes the persistent exposure state) and `tonemap` (reads `hdr` and the exposure state; writes the presentable target; a side effect). Compiling it SHALL order the passes, cull the histogram pass under fixed exposure, compute lifetimes and alias groups, and reject a read without a producer (REND-012). The executor SHALL record passes in the compiled order and SHALL compile again only when the frame shape (size or exposure mode) changes, not every frame.

Verify: A
Status: active
Source: ADR 0042 points 1 and 2, ADR 0013, REND-012, CON-03

#### Scenario: Adaptive frame
- **WHEN** the adaptive lit frame is compiled
- **THEN** the order is terrain, exposure-histogram, exposure-adapt, tonemap and nothing is culled

#### Scenario: Fixed exposure
- **WHEN** the fixed-exposure lit frame is compiled
- **THEN** the histogram pass is culled by the compile and the order is terrain, exposure-adapt, tonemap

#### Scenario: Recompile on a shape change only
- **WHEN** three frames of one shape, a frame of another exposure mode and three more of it are executed
- **THEN** the graph is compiled twice

### Requirement: REND-020 Frame-target share
The bytes of the frame targets that the compiled frame graph allocates (transient resources unaliased, plus persistent ones such as the exposure history), besides terrain's pools and the presentable target, SHALL NOT exceed `40 B x pixels + 32 MiB` of the internal resolution (`frame_target_share_bytes`; ADR 0048, an amendment to ADR 0033's remainder). The count SHALL come from the compiled graph (`CompiledGraph::frame_target_bytes`), and the bytes the renderer allocates for the lit path SHALL equal it. A frame that grows beyond the share SHALL fail the test with the byte counts.

Verify: A
Status: active
Source: ADR 0042 point 5, ADR 0033, ADR 0048, CON-20

#### Scenario: 1440p numbers
- **WHEN** the adaptive lit frame at 2560x1440 is compiled
- **THEN** the scene target is 29,491,200 B, the depth target 14,745,600 B, the histogram 256 B, the exposure history 16 B, the total 44,237,072 B and it is within the share of 181,010,432 B

#### Scenario: A frame that outgrows the share
- **WHEN** a frame declares eight further HDR-sized targets
- **THEN** its count exceeds the share

#### Scenario: Allocation equals the graph
- **WHEN** the lit path is enabled on a renderer
- **THEN** the bytes it holds for the scene target, depth, histogram and exposure history equal the compiled graph's frame-target bytes

### Requirement: REND-021 Exposure and tone-mapping maths
`frame::look` SHALL hold, in f64 with `libm`, the formulas the lit path's shaders implement: the exposure scale `1 / (1.2 x 2^EV100)`, the pixel EV `log2(8 L)`, the pre-exposure factor from scene units (one unit is the look's `units.solar_illuminance_lux`, 126,600 cd/m2 at the Earth default, LOOK-003), the 64-bin histogram over the look's axis (EV -10 to +20 at the default; pixels without light not counted, out-of-range pixels in the end bins), the mean EV of the band between the look's fractions (40 % and 90 % at the default) of the counted pixels, the target clamped to the EV100 range (held when nothing is counted), the adaptation step (linear in EV, at most `speed_up x dt` up and `speed_down x dt` down, snapping on a reset), the ACES filmic fit (Narkowicz 2015), the sRGB transfer function and the 8-bit tone-mapped pixel. The look's values come from the look file (LOOK-001) and the functions that take none are the Earth default. Class: presentation data (ADR 0035); the pre-exposure factor is decided in f64 and rounded once to f32 for the shader.

Verify: A
Status: active
Source: ADR 0038 points 1-4, ADR 0048 points 2-4, ADR 0042 point 6, ADR 0035, CON-03, CON-14

#### Scenario: Known answers
- **WHEN** the units, bins, band means, clamps, adaptation steps, ACES and sRGB are evaluated on hand-worked inputs
- **THEN** each equals its worked answer (EV 0 exposure 1/1.2, L = 1 is EV 3, ACES(1) = 2.54/3.16, sRGB(0.5) = 0.735357)

#### Scenario: Adaptation
- **WHEN** the EV is 10, the target 16 and dt 0.5 s with the default speeds, and the target 2 for the other direction
- **THEN** the EV becomes 11.5 and 9.5, a close target is reached without overshoot, no time means no change and a reset snaps

### Requirement: REND-022 Exposure histogram and adaptation on the GPU
The exposure histogram pass SHALL count the scene target's pixels into the 64 bins of REND-021 and the adaptation pass SHALL compute the band mean, the clamped target and the adapted EV from the histogram, with `dt`, the speeds, the clamps, the histogram axis and band, the mode and the pre-exposure taken from the plan (no wall clock, no readback; the axis and band are the look's, LOOK-006). Class: presentation data. Bound: the GPU histogram equals `look::histogram_of` bin for bin on pixels away from bin edges (on a terrain frame at most 0.5 % of the counts may sit in a neighbouring bin, the total is exact); the adapted and target EV equal the CPU step within 1e-4 EV; the same plan sequence gives the same exposure bit for bit; fixed exposure ignores the scene and the history. The adaptation sums bin counts in f32, exact below 2^24 counted pixels (4K is 8.3 million); larger internal resolutions need an integer running sum first. A plan with non-finite or negative time, a reversed range, negative speeds or a non-unit sun is refused with the value named (REND-024).

Verify: B
Status: active
Source: ADR 0038 points 2 and 4, ADR 0048 point 4, ADR 0042 point 6, CON-04

#### Scenario: Histogram against the reference
- **WHEN** an image of pixels at bin centres, black pixels and out-of-range pixels is written to the scene target and the adaptive frame runs
- **THEN** every bin equals the CPU reference and a second frame gives the same counts

#### Scenario: Adaptation sequence
- **WHEN** scenes of EV 8, 14, 14, 3, 3, 25, 25 and black follow each other with different dt
- **THEN** the GPU EV and target match the CPU chain frame by frame and never pass the range maximum

#### Scenario: Fixed exposure
- **WHEN** a fixed-exposure frame runs on a dark and on a bright scene
- **THEN** the EV and the target are the fixed EV and the factor is 1

### Requirement: REND-023 Tone map
The tone-map pass SHALL read the scene target and the exposure state, expose the value (`written x 2^(pre - ev)`), apply the ACES filmic fit and the sRGB transfer function in the shader, and write an `Rgba8Unorm` target (not an sRGB format), so the window and the goldens encode identically. A false-colour mode SHALL show the exposed luminance in whole-stop bands (`look::EV_BAND_COLORS`) as the tone map's debug view (CON-18). Class: presentation data. Bound: within 1/255 per channel of `look::tonemap_pixel` on a ramp image at several exposures and through the adaptive exposure the GPU reports; band colours exact.

Verify: B
Status: active
Source: ADR 0038 points 3-5, CON-18

#### Scenario: Ramp
- **WHEN** a log ramp from 1e-3 to 50 is tone mapped at EV 10 and EV 13.5
- **THEN** every channel is within 1/255 of the f64 reference

#### Scenario: Exposure difference
- **WHEN** the adaptive exposure lands on an EV other than the pre-exposure
- **THEN** the image equals the reference at `2^(pre - ev)` times the written values

#### Scenario: EV bands
- **WHEN** greys at the middle of each band, a clipped value and black are shown in the false-colour mode
- **THEN** each shows its band colour, the clipped value band 0 and black stays black

### Requirement: REND-024 Lit plan
The `FramePlan` (`TerrainPlan`) SHALL carry the lit path's decisions in an optional `LitPlan`: the exposure mode (fixed with its EV100, or adaptive with its range, speeds and the CPU-chosen pre-exposure EV100), `dt` in seconds from the injected clock, the sun direction, a reset flag, the tone-map mode, the look's scene-unit scale and histogram axis and band (LOOK-003), and the base shading's albedo table, water level and debug view (ADR 0039). The sun direction comes from the look's sun (LOOK-001) or from a time of day: `sun_direction(hours, declination)` is the unit vector `(cos d cos l, cos d sin l, sin d)` with `l = -15 degrees (hours - 12)`, in COORD's axes (+z north pole, +x at longitude 0, +y at 90 degrees east), decided in f64 with `libm` and rounded once to f32. Its text form SHALL be exact (floats as bit patterns) and round-trip, a plan without it SHALL keep its text and hash, and a lit plan made with the Earth-default look SHALL keep the text it had before the look file existed (the look's tokens, the albedo table, the water level and the debug view are written, in that order, only when they differ from the default). A plan with it SHALL be refused by a renderer that has not enabled the lit path, with an error naming `enable_lit`.

Verify: A
Status: active
Source: ADR 0042 point 3, ADR 0048 point 3, CON-03, CON-04, CON-19

#### Scenario: Round trip
- **WHEN** a fixed and an adaptive lit plan are written to text and read back
- **THEN** they are equal

#### Scenario: Debug path unchanged
- **WHEN** a plan has no lit part
- **THEN** its text has no `lit` line and equals the text it had before, and a lit plan has exactly one more line

#### Scenario: Not enabled
- **WHEN** a lit plan is rendered by a renderer without the lit path
- **THEN** the render fails with an error that names `enable_lit`

#### Scenario: Sun from a time of day
- **WHEN** the sun is taken at 12:00, 18:00, 6:00 and 0:00 at declination 0, at declination 90, and at assorted times and declinations
- **THEN** it is +x, -y, +y and -x, +z, a unit vector each time, and a time 24 hours later gives the same direction

### Requirement: REND-025 Lit terrain radiance
The terrain fragment stage of the lit path SHALL compute radiance in units of the sun's illuminance as Lambert diffuse `albedo x max(n . sun, 0) / pi` of the normalised shading normal under the plan's sun direction (ADR 0039 point 4: no ambient, no terrain shadows, no specular; the sky's light comes with ADR 0040), where the albedo is `AlbedoParams::albedo` of the climate at the fragment, the cosine of the slope (the angle between the normal and up) and the height above the plan's water level W (ADR 0039 point 3, LOOK-001's `albedo` table): ground from the climate (desert, grass, forest, tundra), bare rock on steep ground, snow by temperature and slope, a beach band above W and the seabed below it. It SHALL multiply by the plan's pre-exposure factor and clamp each channel to [0, 60000] (ADR 0038 point 1). Debug views (CON-18) of the plan SHALL show the albedo alone, the lighting alone (a white surface), the temperature and the precipitation (false colours, written as if lit by a sun facing the surface). Class: presentation data (ADR 0035). Bound: per channel `|GPU - f64 reference| <= max(2e-4 |v|, 2e-5 x factor)` on probe inputs that include unnormalised normals, climates over the whole range, heights above and far below the water, grazing and back-facing suns, two albedo tables and pre-exposures that reach the clamp (the reference sees the f32 inputs and the f32 table the shader receives).

Verify: B
Status: active
Source: ADR 0038 point 1, ADR 0039 points 1, 3 and 4, ADR 0035, ADR 0043, CON-11, CON-18

#### Scenario: Probe set
- **WHEN** 970 probe inputs in all five views are evaluated by the shader's own function (`shade_lit`) and by `look::shading::terrain_radiance` with `look::written_value`
- **THEN** every channel is within the bound, and the set covers clamped, dark and lit results and every view

#### Scenario: Known answers
- **WHEN** a flat warm dry surface, a humid one, a wet one, a steep one, a cold flat one, a cold steep one, a shore and a seabed are evaluated, and a surface faces the sun, faces away or is tilted by 60 degrees
- **THEN** the albedo is exactly the desert, grass, forest, rock, snow, rock, beach and seabed colour, halfway through a transition it is halfway between the two colours, and the radiance is albedo/pi, zero and half

### Requirement: REND-026 Lit terrain frame
A plan with a `LitPlan` SHALL be drawn into the `Rgba16Float` scene target with the same geometry and the same reversed-Z `Depth32Float` test as the debug path (the same pixels covered), every written value finite and within [0, 60000], and tone mapped into the renderer's colour target; the debug path of the same renderer SHALL stay byte-identical, before and after lit frames and compared with a renderer that never enabled the lit path. A lit frame at a fixed exposure SHALL be a pure function of its plan; the sun and the exposure SHALL come from the plan. Lit frames of the orbit, aerial and ground views, the EV-band view and the base shading's debug views (temperature, precipitation, albedo and lighting) SHALL match the adapter's goldens (class lit; TEST-022, TEST-036). After `enable_lit` no frame SHALL create a pipeline, target or ring.

Verify: C
Status: active
Source: ADR 0038, ADR 0042 points 1-3, ADR 0061, ADR 0048, CON-12, CON-18, REND-017

#### Scenario: Coverage and range
- **WHEN** the scripted views are drawn on the lit path and on the debug path
- **THEN** the covered pixels are the same, every scene value is finite and at most 60,000, and the planet is lit

#### Scenario: Debug path untouched
- **WHEN** a renderer with the lit path draws a debug plan before and after lit frames
- **THEN** the image equals the image of a renderer that never enabled the lit path

#### Scenario: Pure function of the plan
- **WHEN** the same fixed-exposure lit plan is drawn twice, then with another sun, then two stops darker
- **THEN** the first two images are equal, the sun changes the image and the darker exposure is darker

#### Scenario: Formats
- **WHEN** the lit path's targets are inspected
- **THEN** the scene target is `Rgba16Float`, the tone-map output `Rgba8Unorm` and the depth `Depth32Float` with the Greater test

### Requirement: REND-027 Climate and slope reach the fragment stage
The tile's climate field (GEN-021) SHALL reach the fragment stage: the fragment stage reads the 9 x 9 texture from the climate block of the tile's slot (GEN-025; the tile reference and the block are visible to the fragment stage) and interpolates it bilinearly at the fragment's tile coordinates. The node record (208 bytes) SHALL carry the unit up vector of the tile origin and the inverse of its distance from the planet centre, decided in f64 and rounded once; the fragment stage SHALL form up as `normalize(up0 + local x inv_r)` from the tile-local position, so no planet-radius value reaches the stage (CON-11 v1.4). Class and bound of each attribute (ADR 0035): the stored climate texture is a **field** (GEN-021: the GPU decode is exact); the climate at a fragment is presentation data, the bilinear interpolation of the stored texels (the climate views agree with the colour of the analytic climate at the point under each pixel with a mean error of at most 0.02 on the orbit view and 0.005 on the 20 km view (at most 3 % of pixels, the limb, more than 0.1 off)); the up vector is presentation data within 5e-7 rad of the exact radial direction at every tile-local offset up to 1 km from the origin of a tile at 8,848 m or at -11,000 m, anywhere on the planet; temporal stability at 1 m (CON-11): the slope input changes by at most 3e-7 between fragments 1 mm apart at the worst case (a finest-level tile far from the origin), the albedo selected from it by at most 1/255, two tiles that cover one point agree about up within 5e-7 rad, and the shading depends only on tile-local quantities, never on the camera; the albedo, the radiance, the sun direction (f32, 6e-8) and the water level (f32) are presentation data with the bounds of REND-025 and LOOK-003.

Verify: B, C
Status: active
Source: ADR 0039 points 2 and 3, ADR 0035, ADR 0043, ADR 0033 point 4, CON-11, CON-14, REND-016, GEN-025, ADR 0062

#### Scenario: Up vector
- **WHEN** 1,584 tile origins and offsets, anywhere on the planet and at 8,848 m, -11,000 m and sea level, are evaluated by the shader's `up_at` and compared with the exact radial direction
- **THEN** every angle is at most 5e-7 rad (measured 1.4e-7) and every vector is a unit vector

#### Scenario: Stability at 1 m
- **WHEN** 1,001 fragments 1 mm apart along three axes of a tile at 8,848 m are evaluated against a surface at the middle of the soil-to-rock transition
- **THEN** the slope input changes by at most 3e-7 and the albedo by at most 1/255 between neighbours, and two neighbouring tiles agree about up at one point within 5e-7 rad

#### Scenario: Climate views
- **WHEN** the temperature and precipitation views of the orbit and the 20 km view are drawn
- **THEN** their colours are the colours of the world's climate at the point under each pixel within the bounds above

#### Scenario: Formats
- **WHEN** every view is drawn with the tiles in different slots of the pool (the only tile format since ADR 0062 leaves no second format to compare)
- **THEN** the pictures are identical, so the shading does not depend on where a tile sits

### Requirement: REND-028 Per-pass GPU timestamps
A timed frame (`TerrainRenderer::render(.., true)`) SHALL report the GPU time of every pass of the frame it ran in `FrameTiming::passes`: the terrain pass, the water pass, the atmosphere's sky-view and aerial-perspective products and its composite, the exposure histogram and adaptation passes and the tone map, each from its own pair of timestamp queries, plus the scene copy (the gap between the end of the terrain pass and the start of the water pass) and the frame's GPU span (first timestamp to last). A pass the frame did not run (no water, no atmosphere, fixed exposure culling the histogram) SHALL report nothing, a zero or negative interval SHALL report nothing instead of 0 ms, and an adapter without timestamp queries SHALL report nothing at all. The terrain and water passes SHALL keep their queries, so `gpu_ms` and `water_gpu_ms` equal the terrain and water entries. Timestamps SHALL change no pixel, no draw and no pipeline, and the passes SHALL not be timed on a frame that is not timed (`render(.., false)`, `submit`). The static tables' upload is a queue write on a body's load and no pass of a steady frame, so it has no entry.

Verify: A, C
Status: active
Source: M5 item 8 (status.md), ADR 0042 point 2, ADR 0034, TEST-012, REND-017

#### Scenario: Every pass of a lit frame
- **WHEN** a timed lit frame with the atmosphere, water and adaptive exposure is drawn on an adapter with timestamps
- **THEN** all eight passes and the scene copy have a time above zero, the span is at least the longest pass, `gpu_ms` is the terrain entry and `water_gpu_ms` the water entry, and the pixels equal those of an untimed frame

#### Scenario: Passes that did not run
- **WHEN** a debug frame, a lit frame without the atmosphere and with fixed exposure, and a lit frame without water are timed
- **THEN** the debug frame has the terrain pass alone, the second has no sky-view, composite or histogram entry but the adaptation and the tone map, and the third has no water entry and no scene copy

#### Scenario: From ticks
- **WHEN** resolved ticks of passes 2 ns apart are converted, with a pass of zero interval and a water pass starting before the terrain pass ended
- **THEN** the times are the ticks times the period, the scene copy is the gap, the span runs from the first to the last timestamp, and the zero interval and the negative gap report nothing

### Requirement: REND-029 Device diagnostics
A `GpuContext` SHALL install on its device, before any resource exists, a device-lost callback and an uncaptured-error handler.
- A lost device (any reason other than a deliberate destroy) SHALL print `wgpu device lost (<reason>): <message>` on stderr and be readable as `GpuContext::device_lost`.
- The first uncaptured error SHALL print `first wgpu error: <description>` on stderr and be readable as `GpuContext::first_error`; later ones SHALL not replace it.
- The uncaptured-error handler SHALL then panic with wgpu's own message (`wgpu error: <description>`), as the default handler does.
- The handlers SHALL change no pixel, draw, pipeline or plan.

Verify: A, C
Status: active
Source: perf window 8 (dive on the lit path, `Buffer with 'globals' label is invalid`), TEST-012

#### Scenario: A destroy is no loss
- **WHEN** the lost-device line is built for the reason `Destroyed` and for another reason
- **THEN** the first gives no line and the second a line with the reason and the message

#### Scenario: The first error is kept
- **WHEN** a buffer with an invalid descriptor is created on a context
- **THEN** `first_error` holds the description, stays the same after a second error, and the creating call panicked with `wgpu error:`

### Requirement: REND-030 The shore ramp follows the mesh's tolerance
The lit terrain's albedo goes from the seabed to the shore over a ramp of height above the water. The ramp at a fragment SHALL be `max(shore_blend_m, shore_ramp_at(per_m, d))` tall, where `d` is the fragment's distance from the camera, `per_m = SHORE_RAMP_K * tau_px * pixel_angle` with `pixel_angle = 2 tan(fov_y / 2) / viewport_h_px` (`LodParams::shore_ramp_per_m`, carried once per plan as `TerrainPlan::shore_ramp_per_m` and in the plan text as `shore_ramp_per_m`; a plan without it reads 0 and ramps by the look's blend alone) and `shore_ramp_at(per_m, d) = min(per_m * d, SHORE_RAMP_MAX_M)` (`planet_frame::lod::shore_ramp_at`, 8 m cap). The ramp SHALL be a function of the fragment's distance and of nothing about the node that draws it: the same ground point has the same ramp, and so the same albedo, under a tile of any level (the cover, stale plans and prefetched tiles draw tiles the exact plan would not; a ramp that depended on the drawn level stepped the albedo of the whole shallow seabed at every tile border between levels). The shader forms `d` as the length of the node's camera-relative origin plus the fragment's tile-local position (camera-relative f32, no planet-radius value) and reads `per_m` from the globals; the same width SHALL enter the f64 reference `AlbedoParams::albedo_with_ramp` and the shader's `albedo_of`, so that `oracle_lit_shading` pins them together.

Derivation of the width (no look number, no screen derivative). A node is drawn only closer than its parent's split distance, where the parent's sample spacing projects to `tau_px` pixels: `tau_px * pixel_angle * split(level - 1) = s(level - 1)` whatever the field of view, the viewport and `tau_px` are. So `tau_px * pixel_angle * d` is the world size of the selection's screen-space tolerance at distance `d`, and the height of one ground point differs between a node's mesh, its parent's mesh and the morph between them by a fraction of it. A ramp narrower than that makes an albedo that differs by a factor of eight in luminance between seabed and beach (0.06 against 0.52) flash with decimetre changes of height between frames. `SHORE_RAMP_K` is the smallest factor found that holds the terrain's own shading flash at the shore of both coastal flights under 0.5 %: on the flights of `gpu_water` (a 1:50 beach, tau 4 px) k = 1 leaves 4.3 % of the glide's shore pixels flashing, k = 1.5, 2 and 3 leave none; 2 is taken. At the far end of a node's drawing range the ramp is `K s(level - 1)` (up to the cap), at the near end half of it; k = 2 was calibrated with the former per-node rule, which blended from `K s(level - 1)` to twice that across the morph, so the distance rule is about 0.5 to 0.7 of it pointwise, and the flights (the scenario "No flash on the coastal flights") are the evidence for it, not the derivation. The cap of 8 m is four absorption lengths of the water pass (0.5 per metre by default: 98 % absorbed), below which the seabed's own colour is hidden whatever the ramp is; the flights need 3 m; without the cap the coarse view of an orbit asks for kilometres and the deep seabed takes the beach's colour in terrain-only views. Near the camera `tau_px * pixel_angle * d` is centimetres and the ramp stays at the look's `shore_blend_m`, 1 m by default; it widens only where the mesh cannot hold 1 m of height. The seabed and the beach colours, the beach band above the water, and every other term of the albedo are unchanged.

Verify: A, B, C
Status: active
Source: M5 gate tracked item "terrain shading at the beach flashes" (6.3 % of the shore-band pixels of the glide and 36.8 % of the descent flashed by more than a quarter between frames with no water drawn), the attribution of 2026-10-18 (one term: the 1 m seabed-to-shore ramp), ADR 0039 point 3, REND-026, SPIKE-16 (the level-dependent ramp as the cause of the dark patches while the camera moves)

#### Scenario: The width
- **WHEN** the tolerance is 0, 0.3, 7.5 m or unreadable
- **THEN** the ramp is the look's blend (1 m), the blend, 7.5 m and the blend; the width never falls below the blend and never shrinks as the tolerance grows

#### Scenario: The node's tolerance
- **WHEN** the ramp is taken at the far end of the drawing range of a node of any level 1 to 22 (the parent's split distance), for two different tau, viewport and field-of-view settings
- **THEN** it is `SHORE_RAMP_K` times the parent's sample spacing up to 8 m, for both settings, `per_m` is `SHORE_RAMP_K * tau_px * pixel_angle`, the ramp is 0 at the camera and 8 m far away, and at 40 m it is below the blend

#### Scenario: Continuous across a level border
- **WHEN** the albedo of a seabed point 2.5 m under the water is taken at a fragment distance from 5 m to 20 km as a tile of any level 16 to 22 would draw it, and the ramp is walked along the ground in 5 m steps
- **THEN** the albedo is the same whichever level draws it (under the former per-level ramp it was 4.9 times brighter under L17 than under L19), and the ramp grows with the distance by at most `per_m` per metre without a step

#### Scenario: Shader and reference
- **WHEN** probes sit at the foot, the middle and the top of a ramp of 8 m and at fragments with random distances and growths up to the cap
- **THEN** the shader's radiance equals the reference's within the declared bound of `oracle_lit_shading`

#### Scenario: No flash on the coastal flights
- **WHEN** the glide and the descent over a 1:50 beach are drawn with the plan's ramp
- **THEN** at most 0.5 % of the shore-band pixels change their terrain-only luminance by more than a quarter between three consecutive frames

### Requirement: REND-031 Reference renderer
`render` SHALL offer a test-only reference renderer (`render::reference`, behind the cargo feature `reference-renderer`, ADR 0062 point 1) that draws a `TerrainPlan` from CPU `TileMesh`es in the debug views, with one pipeline and one vertex buffer per tile, so that the compact renderer's image tests keep an independent geometry source. It SHALL have no pools, no fallback, no lit path, no atmosphere and no water, and SHALL refuse a plan with a lit part. It SHALL NOT be a second production path: the feature is off by default and no product crate and no non-dev dependency edge enables it (BUILD-001).

Verify: C
Status: active
Source: ADR 0062, SPIKE-12 Q4, REND-016

#### Scenario: Reference against the compact renderer
- **WHEN** the ten scripted views are drawn in the level, relief and normals views, once by the reference renderer from the meshes and once by the compact renderer from the same tiles
- **THEN** at most the tolerance of REND-016 of the pixels differ (the model error of the compact base surface), and the compact path is exercised on more than 100 tiles

#### Scenario: Lit plan refused
- **WHEN** a plan with a lit part is given to the reference renderer
- **THEN** it fails with an error naming the missing lit path, and draws nothing

### Requirement: REND-032 Stale while revalidating
A node of the drawn plan whose tile is resident with a mesh key other than the plan's (an edit changed what the tile reads and its refresh, STRM-026, is on the way; the same holds for a loop that keeps its tiles across any change of the field) SHALL be drawn from the resident buffer as it is, not replaced by an ancestor: the drawn plan names the resident key and the resident origin, so the renderer's key check (a plan that expects a key other than the buffer's fails with the tile id) is met, the node counts as substituted and the plan is of kind `Stale`, never `Exact`. This is an exception to the ancestor substitution of STRM-016, which applies to tiles that are not resident. An edited place SHALL therefore never go coarser or blank while it reloads, and when the refresh has replaced the tile the next frame SHALL be the exact plan of the new field again.

Verify: A
Status: active
Source: SPIKE-13 variant B and D (evicting the edited tiles drew 490 nodes as ancestors for the whole interval), M6 plan row E1

#### Scenario: The edited place stays as fine as it was
- **WHEN** an edit makes the plan expect new keys for resident tiles and their refreshes have not finished
- **THEN** every node of the drawn plan is the tile the plan named, with the resident key, no node is replaced by an ancestor or blank, and the plan is `Stale`

#### Scenario: Exact again
- **WHEN** the refreshes have replaced the tiles
- **THEN** the drawn plan equals the synchronous plan of the new field

### Requirement: REND-033 Edit preview
A draw plan SHALL carry (in the `DrawPlan` of the frame) a list of at most 32 preview stamps, each a signed stamp (GEN-026: kind, anchor, reach, parameter, bound) with a sign of +1 or -1, and a flag per node: a node is previewed when its resident tile is stale (REND-032), or when the edit in flight named its tile (the footprint find, `ManagedLoop::edit`) and no plan made after that edit has been delivered yet, and the reach of a preview stamp touches the tile's cap. A tile the edit named is drawn as it is and counts as substituted until such a plan arrives, whatever the mesh key of the older plan says, so the preview shows in the frame after the edit and does not wait for the plan worker; from the delivery of that plan on, the keys decide (REND-032). The list SHALL be computed from the footprint items the stale tiles were built from (the oldest state any resident tile shows) and the current items: an item only in the current set is +1, an item only in the old set is -1, a changed item is both, an unchanged one is absent, so a drag of many steps is one delta from the first state; a removed plateau is not listed (the flattened ground is not in the stale tile) and a changed plateau is listed as its new state. The delta SHALL have at most 16 stamps (`MAX_DELTA_STAMPS`); a longer one SHALL keep the units nearest to the camera, a unit's entries together or not at all (a unit is an item's stamp, or one stroke of a brush item, VIEW-036), and report how many entries it left out. The compact vertex stage SHALL add, for a previewed node, the delta of each preview stamp, times its sign, to the height of the vertex along the tile's up in the order of the list, with the profiles of GEN-026 on the distance from the anchor measured in the tangent plane of the vertex's own radial up (so two tiles sharing a border vertex agree) in camera-relative f32 arithmetic (the anchor is rounded once from f64 on the CPU; no planet-radius value in the shader, CON-11), and SHALL displace the neighbours that make the parent-grid target the same way, so the morph target is the displaced parent grid; it SHALL correct the normal by the slope of the summed delta and lean it to the up by how flat a plateau makes the ground; a plateau uses the vertex's own height as the running height. The stage SHALL loop over the active count of the list (the count the globals carry), not over the table, so that a frame with no stamp or a short list costs what it holds. A transient preview (VIEW-036), the stamps of something being made such as a brush stroke under the pointer (`ManagedLoop::set_transient_preview`, `draw_plan_transient`), SHALL flag every node drawn from a resident tile whose cap a transient stamp touches, whether the tile is stale or exact, and the draw plan SHALL carry the delta's stamps and then the transient ones, at most 32 (the delta's at most 16 and the transient ones at most 16, so that none is cut); it does not change the kind of the frame. A node SHALL apply only the transient stamps (`transient_only`, node record `morph.z` 2) when its tile is exact (it already holds the edit the delta stands for) or when no stamp of the delta touches it; the globals' `extra.w` SHALL carry the active count in its low 6 bits and 64 times the number of the delta's stamps above them, and the vertex stage SHALL start such a node's loop after the delta's stamps. A smooth brush stroke is not previewed (a known limit, VIEW-036). With no preview stamp the stage SHALL not be entered: every frame is bit-identical to a frame of a build without the preview. A node whose resident key equals the plan's SHALL not be previewed, unless the plan predates an edit that named its tile, so the exact tile replaces the preview without a step in the list.

Verify: A, C
Status: active
Source: SPIKE-13, ADR 0066 points 3 and 5, M6 plan row E3, window 14 (2026-10-19)

#### Scenario: Delta
- **WHEN** a stamp is added, removed, moved, and changed in size between two item sets
- **THEN** the list holds the +1 and -1 entries named above, none for an unchanged item, and at most 16 of them, nearest first

#### Scenario: Previewed nodes
- **WHEN** a draw plan has resident tiles with the plan's key, with an older key under a preview stamp, and with an older key away from it
- **THEN** only the second kind is flagged, and a node flagged earlier whose exact tile arrived is not

#### Scenario: The frame after the edit
- **WHEN** an edit names resident tiles under a preview stamp and the next frame is begun before any plan made after the edit is delivered, then the jobs finish
- **THEN** that frame flags the named tiles under the stamp (and only tiles the edit named), is not exact and counts them as substituted, and once the exact tiles are resident the frame is exact and nothing is flagged

#### Scenario: Displacement
- **WHEN** a scene whose nodes are all flagged is drawn on the compact path with no preview stamp, with a stamp on unflagged nodes, with a stamp, with a stamp and its removal, and with a stamp of a smaller reach
- **THEN** the first two frames equal the frame of the build without the preview, the third differs, the fourth equals the first, and the fifth changes no more pixels than the third; the shader's profiles are those of GEN-026 (a scan of the shader text) and the stage is entered only for a flagged node with a stamp

#### Scenario: Close to the exact tiles
- **WHEN** a hill, a crater and a plateau are added to a world, and the Height view is drawn from the stale tiles of the unedited world with the stamp as preview, and from the exact tiles of the edited world planned for it
- **THEN** the preview frame differs from the exact frame by more than three grey levels in at most 5 % of the pixels the edit changes, and the same holds for the Normals view with steeper stamps (more than six grey levels)

### Requirement: REND-034 Procedural ground detail
The lit terrain shader SHALL add a procedural detail term to the albedo and the shading normal, driven by the plan's strength (LOOK-009) and the material weights (LOOK-008): four noise octaves with cells of 16 m, 2 m, 25 cm and 3 cm, an octave fading out while a pixel covers 0.06 to 0.25 of its cell. The octaves SHALL be anchored to the world: a node record holds the integer cell of the tile origin at 1/32 m and the fraction, both decided in f64 on the CPU, in the unused fourth lanes of its grid phases (the record does not grow), and an octave's cell is that integer shifted right by the octave's bits (9, 6, 3 and 0), so two tiles (or two levels of detail) read the same noise at the same ground point; the cell is an exact integer used only for shifts and hashing and no planet-radius float is formed in the shader (CON-11), and the cell range is 6.7e7 m from the centre. The noise SHALL be planar value noise with analytic gradients on the plane of the lattice the surface normal lies closest to (the planes of the axes within 0.12 of it weigh in by weights continuous in the normal), the lattice of every other octave turned by 45 degrees by an integer matrix (so the anchoring stays exact). Its cost SHALL be at most +1.0 ms of GPU time over the detail off at 2560x1440 at the eye heights 1.8 m and 0.5 m (G2c; the window of M7 measures it). With the strength 0 the picture SHALL be what it was before the term, bit for bit; the climate and lighting-only views SHALL not show the albedo term. Per material the procedural term is one source of the albedo multiplier and the normal tilt: a later texture source (a close-range photo layer) SHALL be able to replace or blend into it without a change of the weights function or the node record. Presentation only: heights, collision and picking are untouched (ADR 0028).

Verify: A, C
Status: active
Source: M7 G2a (opus-planner, 2026-10-21), SPIKE-27 option A, owner decisions of 2026-10-21 (the photo layer later, with a seam), ADR 0035

#### Scenario: The anchor
- **WHEN** two node records have origins 64 m apart, one at a negative planet-radius coordinate
- **THEN** their cells differ by 2048 at 1/32 m and by the matching shift at every octave, their fractions are equal, and each record's cell plus fraction recovers its origin to 1e-6 m

#### Scenario: Off, on, and the table
- **WHEN** a ground-level scene is drawn with the strength 0 (with any material table), with 1, with a table of no material, with a table of rock alone and of soil alone, in the temperature and precipitation views, and with the tiles in other slots
- **THEN** the strength 0 pictures are equal whatever the table; the strength 1 picture changes more than a fifth of the ground by more than 2 %, is equal on a second render and keeps its mean brightness within a factor of two; the no-material table equals the off picture; rock alone and soil alone differ on more than a fifth of the ground; the climate views equal their off pictures; other slots give the same picture

### Requirement: REND-035 Specular term of the ground
With the ground detail on (REND-034, strength above 0) the lit fragment stages SHALL add the sun's specular reflection to the detailed ground: GGX distribution with Smith-Schlick visibility and Schlick Fresnel of the plan's `f0` (LOOK-010), times the cosine, with the roughness of the point from the plan's gloss table and the same weights as the detail (LOOK-008), scaled by the strength. The sun's illuminance is one unit without an atmosphere and the transmitted, shadowed sun with one. The term SHALL be added to the lit view only (not the albedo, lighting or climate views), SHALL never darken a pixel, and SHALL be absent when the strength is 0 or no material has weight, so a picture with the detail off is what it was bit for bit. Presentation only (ADR 0028, ADR 0035).

Verify: C
Status: active
Source: M7 G2b (opus-planner, 2026-10-21; owner: roughness and shine are in M7), REND-034, LOOK-010

#### Scenario: The highlight
- **WHEN** a ground-level scene is drawn with the detail on and `f0` 0, with the default `f0`, with a smooth and with a rough table, with the detail off and `f0` 1, and in the albedo view with `f0` 1
- **THEN** no channel of the default-`f0` picture is below the `f0` 0 picture and some pixels are brighter; the smooth and the rough tables give pictures that differ from each other and from the default; the detail-off picture equals the one with `f0` 0 and the albedo view equals its picture with `f0` 0

### Requirement: REND-036 Scale references pass
The lit path SHALL draw the scale references it is given (`TerrainRenderer::set_scale_refs`, VIEW-050) as boxes into the terrain pass, with the terrain's depth test, so the terrain hides what lies under it and the boxes hide what lies behind them. A box is shaded by Lambert diffuse `albedo * (max(n . sun, 0) + 0.12) / pi` at the plan's pre-exposure, in the units of the lit terrain. Positions reach the shader relative to the camera in f32 (CON-11); boxes beyond 200 km of the camera are left out. The light is the plan's sun only, with no planet shadow or atmospheric transmittance (day-side use). A frame without references or on the debug path is what it was.

Verify: C
Status: active
Source: owner (2026-10-22, via opus-planner), VIEW-050, REND-026

#### Scenario: Depth and absence
- **WHEN** a red cube is set half way to the ground on the view axis, then one under the ground, then none, and a debug frame is drawn with and without the cube
- **THEN** the first frame differs from the plain one in red pixels, the second and the third equal the plain frame, and the debug frames are equal

### Requirement: REND-037 The materials have their own colours
With the ground detail on (REND-034), the lit fragment stages SHALL give land its materials' albedo in place of the climate's, by the strength: rock a neutral grey, grass a green, sand a pale yellow-brown and soil a dark brown, each times the detail's multiplier, which is 1 on average. Snow, the beach band and the seabed SHALL keep their own albedo, and the debug views and the strength 0 SHALL be unchanged. The rock's cracks and crevices SHALL be darker than its surface and the grass SHALL carry a pattern at the 3 cm octave.

Verify: C
Status: active
Source: owner (2026-10-22, via opus-planner): "I can't really decipher rocks from grass"; REND-034

#### Scenario: Each material reads as itself
- **WHEN** a ground-level scene is drawn with one material alone at strength 1
- **THEN** the mean colour of the grass has green above 1.3 times red and 1.5 times blue, the rock's channels stay within 12 % of each other, the sand is brighter than the soil by a factor of 2, and the soil is darker than the rock

### Requirement: REND-038 The horizon shadow term
The lit terrain fragment stage SHALL multiply the direct sun term by a shadow factor from the horizon (GEN-046): the sun's elevation against the horizon of the fragment's point at the sun's azimuth, with a soft edge from -0.35 to +0.9 degrees around the horizon, taking the horizon textures of the fragment's chunk and the coarse horizon. The position used to sample the maps SHALL be tile-local or camera-relative f32 (CON-11). With no horizon resident the term SHALL be 1 exactly and the output of every existing view SHALL be unchanged.

Verify: C
Status: active
Source: opus-planner (2026-10-24, M8-5), SPIKE-32 step 2, CON-11, CON-18

Detail: When a map arrives a fragment goes at once from that identity to the factor of the horizon (a sun under 0.9 degrees above flat ground dims); the chunk fade (REND-039) does not hide it. The factor is continuous in position (bilinear reads, unit vectors only), and the temporal stability of CON-11 is judged on the stills, not by a test. The term is presentation shading (CON-14): it has no CPU twin beyond the GEN-046 reference, and its declared bound against that reference, measured at texel centres on the software adapter, is: chunk horizon 2e-5 rad, coarse horizon 3e-4 rad, lit factor 8e-3 with a chunk map and 2e-2 with a coarse map (the adapter's `asin` and texture weights on a 1.25 degree soft edge). The maps SHALL be read with bilinear filtering: between texel centres the factor blends neighbouring cells (the reference takes the cell), and across a cube-face edge the clamped sampler stops the blend, leaving a step of one cell's horizon difference along the edge in the far-field mask (a stated limit). Both maps SHALL hold exactly 16 azimuths. The shadow term and the material map together SHALL take at most 1.5 ms of terrain-pass GPU time at 1440p (M8 criterion 5; measured in the architect's window after the material map (M8-6), never in a test; tracked in `docs/status.md`).

#### Scenario: Shader against the reference
- **WHEN** the shader's shadow factor is read back at sampled texels and sun directions
- **THEN** it equals the CPU function of GEN-046 within the stated tolerance, and with shadows off the frame equals the frame of a build without the term

#### Scenario: The view
- **WHEN** the horizon and shadow views are drawn on the close-ground pose over a synthetic horizon ramp
- **THEN** the horizon view is a grey gradient and the shadow view shows lit and shadowed bands at the ridge lines, and the images match their goldens (`lit_ground_horizon_synthetic_ev15`, `lit_ground_shadow_synthetic_ev15`, first goldens waiting for the owner's /bless); the look-review poses at sunset and midday are inspection images (`docs/milestones/M8/horizon-shadows-*`), not goldens

### Requirement: REND-041 The frame's horizon data
`planet_frame::horizonview` SHALL decide, GPU-free, what the renderer takes for the horizon shadow: a chunk's horizon map packed as the GPU reads it (four layers of `256 x 256` texels of four azimuth bytes), the rectangle of a tile inside its chunk, the sun's azimuth and elevation in the chunk's raster frame, the slots of the renderer's array, and one table entry per plan node (`HorizonFrame`). A map that does not fit SHALL be refused, never truncated.

Verify: A
Status: active
Source: opus-planner (2026-10-24, M8-5), decisions 1 and 2 (a texture array, one layer-group per resident chunk slot), CON-03

Detail: `planet_frame::horizonview` SHALL decide, GPU-free, what the renderer takes for the horizon shadow: a chunk's horizon map (GEN-044: `n x n` cells of up to 16 azimuths) packed as the GPU reads it, four layers of `256 x 256` texels of four azimuth bytes (the azimuths `4 L .. 4 L + 3` in layer `L`, a missing one 0), each texel the mean of the map's cells under it and at least one; the rectangle of a tile inside the chunk it lies in (origin and size in the chunk's `[0, 1]^2`, `None` for a tile coarser than the chunk grid); the sun's azimuth and elevation at a chunk's centre in the chunk's raster frame (GEN-046, `+j` north, `+i` east); and the slots of the renderer's array: a fixed number, one per chunk in view that has a map, a re-bake uploaded again into the same slot, the least recently wanted slot given up when none is free, the nearest chunks served first, a released chunk's slot free at once. A map that does not fit is refused, never truncated. For the plan's nodes in order it SHALL build one entry each (`HorizonFrame`): the slot of the node's chunk (`-1` for a tile coarser than the chunk grid or a chunk with no slot, the other numbers then 0), the sun's azimuth and elevation at the chunk's centre, the chunk fade's weight (1 until REND-039) and the tile's rectangle, and the flat table of two `vec4` a node that the shader reads.

#### Scenario: Packing and slots
- **WHEN** maps of 8, 512 and 614 cells are packed, tiles at several levels are placed in their chunks, and slots are assigned over a flight with more chunks wanted than slots
- **THEN** a small map's texel is the cell under it, a larger one the mean of the cells (a constant map stays constant), a bad map is refused, a tile's rectangle is its place in the chunk, the sun's azimuth is a quarter turn toward `+i` and 0 toward `+j`, a re-bake uploads once into its slot, and the stalest slot goes to the nearest new chunk

### Requirement: REND-039 A landed chunk's change fades in the draw
A tile with chunk detail SHALL carry the chunk's change per vertex beside its heights, and the vertex shader SHALL place each vertex, and the parent-grid target it morphs to, at `h_q - (1 - w) x change` along the base normal. The fade `1 - w` SHALL be the fourth lane of grid phase 6 of the node record (an f32, CON-11), the plane read from the frame's tile-reference buffer. With `w = 1` or without a plane the arithmetic SHALL be the old one bit for bit, and at `w = 0` the drawn height SHALL be the base's. The normals stay the final surface's. The CPU mirror (`CompactTile::shade_vertex_faded`) SHALL form the same f32 operations.

Verify: A, C
Status: active
Source: M8 exit criterion 3, ADR 0077 point 7, STRM-039 (the weight), GEN-025, REND-031

#### Scenario: The fade in the draw
- **WHEN** a tile with a change plane is drawn at `w` 0, 0.5 and 1 on the software adapter, and a tile without one is drawn
- **THEN** the drawn heights are the base's, the base plus half the change, and the final heights (the compact renderer's image equals the reference renderer's image of a product mesh of the blended field, which uses its own arithmetic: no pixel differs by more than the test's tolerance of 0.4 %, 0 measured); the tile without a plane is bit-identical to the old draw, and so is a tile with one at weight 1; the mirror's vertices are the final minus fade times change; and the step per frame of a fade over `chunk_fade_ms` at 60 frames per second is at most the weight step times the largest change (0.19 m for the 256 bake's largest entry step of 17.55 m, 0.65 m for the shared store's 58.3 m, which is 0.20 px on screen where it enters: `design.md` 6a; a measurement of the scene, not an assertion of the test)

### Requirement: REND-042 Fade planes
The renderer SHALL write a plane into the frame's tile-reference buffer only for a node whose change is still fading in, at most `FADE_PLANES` (512) a frame, the rest drawing their final height (`TerrainStats::fade_overflow`). A plane that does not hold one value per texel, or holds one that is not finite, SHALL be refused naming its length or texel, and the CPU mirror SHALL form no `fade x change` at fade 0. Planes SHALL be given stickily (`FadePlaneGrants`): a tile holding one keeps it while it fades, a tile left without stays without until its fade ends, a new one is granted in tile-id order while there is room.

Verify: A, C
Status: active
Source: REND-039, M8 exit criterion 3

#### Scenario: Planes
- **WHEN** more tiles fade than there are planes, the lowest id finishes, and a plane holds NaN, inf or too few values
- **THEN** the tiles that held planes keep them and those left without stay without until their fades end, and the bad plane is refused naming its texel or its length

#### Scenario: Known limit at a chunk border
- **WHEN** a tile reads a chunk that is not ready while its neighbour reads only the ready ones
- **THEN** the tile weighs 0 while the neighbour is at its weight, so the two differ along their shared edge by up to the change times the weight difference for as long as the neighbour chunk takes; the skirt (0.08 of the tile edge, about 12 m at level 16 and 49 m at level 14) covers only the steps of the finer levels, not the 58 m of the largest valley, until `m8-chunk-fade-increment`
